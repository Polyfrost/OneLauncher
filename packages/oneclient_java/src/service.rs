use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use oneclient_events::{Choice, EventBus, GroupedProgressSession, Prompt};
use oneclient_net::RequestClient;

use crate::checker::{self, JavaCheckInfo};
use crate::data::{JavaPackage, JavaRuntime};
use crate::error::{JavaError, JavaResult};
use crate::resolve::resolve_java_executable;
use crate::store::JavaStore;
use crate::vendors::{self, JavaVendor};

/// Choice ids for the "Java required" prompt front-ends match on these
pub const JAVA_CHOICE_DOWNLOAD: &str = "java.download";
pub const JAVA_CHOICE_FOLDER: &str = "java.folder";

/// Tells the front-end to show its vendor list rather than another picker
pub const JAVA_VENDOR_HINT: &str = "java-vendor";

pub const INSTALLABLE_MAJORS: &[u32] = &[8, 11, 16, 17, 18, 19, 20, 21, 22, 23];

pub const PROBE_VERSION: u32 = 1;

const _: () = assert!(PROBE_VERSION > 0);

enum JavaPromptAnswer {
	Download,
	PickFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableJava {
	pub major: u32,
	pub package: JavaPackage,
}

pub struct JavaService {
	store: Arc<dyn JavaStore>,
	net: RequestClient,
	events: EventBus,
}

impl JavaService {
	#[must_use]
	pub fn new(store: Arc<dyn JavaStore>, net: RequestClient, events: EventBus) -> Self {
		Self { store, net, events }
	}

	#[must_use]
	pub fn net(&self) -> &RequestClient {
		&self.net
	}

	#[must_use]
	pub fn events(&self) -> &EventBus {
		&self.events
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn list_runtimes(&self) -> JavaResult<Vec<JavaRuntime>> {
		Ok(self.store.list().await?)
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn runtime_for_profile(
		&self,
		java_path: Option<&str>,
	) -> JavaResult<Option<JavaRuntime>> {
		let Some(path) = java_path else {
			return Ok(None);
		};
		let Some(runtime) = self.store.get_by_path(path).await? else {
			return Ok(None);
		};
		self.revalidate(runtime).await
	}

	#[tracing::instrument(level = "debug", skip(self))]
	async fn revalidate(&self, runtime: JavaRuntime) -> JavaResult<Option<JavaRuntime>> {
		if runtime.probe_version == PROBE_VERSION {
			if Path::new(&runtime.absolute_path).is_file() {
				return Ok(Some(runtime));
			}

			tracing::warn!(
				path = %runtime.absolute_path,
				"forgetting recorded Java runtime: the executable is gone"
			);
			self.store.delete_by_path(&runtime.absolute_path).await?;
			return Ok(None);
		}

		match checker::check_java_runtime(runtime.absolute_path.clone()).await {
			Ok(info) => Ok(Some(
				self.persist(Path::new(&runtime.absolute_path), &info).await?,
			)),
			Err(err) if err.is_invalid_installation() => {
				tracing::warn!(
					path = %runtime.absolute_path,
					"forgetting recorded Java runtime: {err}"
				);
				self.store.delete_by_path(&runtime.absolute_path).await?;
				Ok(None)
			}
			Err(err) => {
				tracing::warn!(
					path = %runtime.absolute_path,
					"could not re-probe recorded Java runtime, trusting the record: {err}"
				);
				Ok(Some(runtime))
			}
		}
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn available_versions(&self, vendor: &JavaVendor) -> JavaResult<Vec<AvailableJava>> {
		let Some(provider) = provider_for_vendor(vendor) else {
			return Ok(Vec::new());
		};

		let packages = provider.list_packages(None, &self.net).await?;

		let mut by_major = BTreeMap::<u32, JavaPackage>::new();
		for package in packages {
			let Some(&major) = package.java_version.first() else {
				continue;
			};
			by_major.entry(major).or_insert(package);
		}

		let mut available: Vec<AvailableJava> = by_major
			.into_iter()
			.map(|(major, package)| AvailableJava { major, package })
			.collect();

		available.sort_by_key(|entry| std::cmp::Reverse(entry.major));
		Ok(available)
	}

	#[tracing::instrument(level = "debug", skip(self))]
	pub async fn rescan(&self) -> JavaResult<()> {
		self.register_located(&crate::locate::locate_java().await?)
			.await
	}

	#[tracing::instrument(skip(self))]
	pub async fn add_custom_runtime(&self, folder: PathBuf) -> JavaResult<JavaRuntime> {
		let executable = resolve_java_executable(&folder)?;
		self.register_checked(&executable, None).await
	}

	#[tracing::instrument(skip(self))]
	pub async fn remove_runtime(&self, absolute_path: &str) -> JavaResult<()> {
		self.store.delete_by_path(absolute_path).await?;
		crate::platform::forget_dedicated_gpu(Path::new(absolute_path)).await;
		tracing::info!("removed Java runtime");
		Ok(())
	}

	/// A recorded JRE is kept only as a fallback so a system scan gets its
	/// chance to turn up a JDK of the same major first
	#[tracing::instrument(level = "debug", skip(self, progress))]
	pub async fn prepare(
		&self,
		major: u32,
		search_system: bool,
		auto_install: bool,
		progress: Option<&GroupedProgressSession>,
	) -> JavaResult<JavaRuntime> {
		let recorded = loop {
			let Some(runtime) = self.store.latest_by_major(major).await? else {
				break None;
			};
			if let Some(valid) = self.revalidate(runtime).await? {
				break Some(valid);
			}
		};

		if let Some(runtime) = &recorded
			&& runtime.is_jdk
		{
			return Ok(runtime.clone());
		}

		if search_system {
			let located = crate::locate::locate_java().await?;
			self.register_located(&located).await?;

			// A located runtime only displaces a recorded one by being a JDK
			if let Some((path, info)) = crate::locate::best_for_major(&located, major)
				&& (info.is_jdk || recorded.is_none())
			{
				return self.persist(path, info).await;
			}
		}

		if let Some(runtime) = recorded {
			return Ok(runtime);
		}

		if auto_install {
			return self.download_and_register(major, progress).await;
		}

		self.prompt_and_install(major, progress).await
	}

	#[tracing::instrument(skip(self, progress))]
	pub async fn install_runtime(
		&self,
		major: u32,
		progress: Option<&GroupedProgressSession>,
	) -> JavaResult<JavaRuntime> {
		self.download_and_register(major, progress).await
	}

	#[tracing::instrument(skip(self))]
	pub async fn install_runtime_from(
		&self,
		vendor: &JavaVendor,
		major: u32,
	) -> JavaResult<JavaRuntime> {
		self.install_vendor_runtime(vendor, major, None).await
	}

	#[tracing::instrument(level = "debug", skip(self, progress))]
	async fn install_vendor_runtime(
		&self,
		vendor: &JavaVendor,
		major: u32,
		progress: Option<&GroupedProgressSession>,
	) -> JavaResult<JavaRuntime> {
		let provider = provider_for_vendor(vendor).ok_or(JavaError::PackageNotFound { major })?;
		let package = provider
			.latest_package_by_major(major, &self.net)
			.await?
			.ok_or(JavaError::PackageNotFound { major })?;

		let owned = progress.is_none().then(|| {
			GroupedProgressSession::start(&self.events, format!("Installing Java {major}"))
		});
		let session = progress.or(owned.as_ref()).expect("session present");

		let executable = provider
			.install_package(&package, &self.net, &self.events, Some(session))
			.await?;

		self.register_checked(&executable, Some(major)).await
	}

	async fn prompt_and_install(
		&self,
		major: u32,
		progress: Option<&GroupedProgressSession>,
	) -> JavaResult<JavaRuntime> {
		let prompt = Prompt::new(
			"Java required",
			format!(
				"No Java {major} runtime was found. Download it automatically, choose an existing installation folder, or cancel?"
			),
		)
		.option(
			Choice::primary(JAVA_CHOICE_DOWNLOAD, "Download").picks_selection(JAVA_VENDOR_HINT),
			JavaPromptAnswer::Download,
		)
		.option(
			Choice::new(JAVA_CHOICE_FOLDER, "Choose folder")
				.picks_folder(format!("Select a Java {major} installation folder")),
			JavaPromptAnswer::PickFolder,
		)
		.dismiss("Cancel");

		let Some(chosen) = self.events.ask(prompt).await? else {
			return Err(JavaError::Cancelled);
		};

		match chosen.value {
			JavaPromptAnswer::Download => match chosen.selection() {
				Some(vendor) => {
					let vendor = JavaVendor::from_str(vendor)
						.unwrap_or_else(|_| JavaVendor::Other(vendor.to_string()));
					self.install_vendor_runtime(&vendor, major, progress).await
				}
				None => self.download_and_register(major, progress).await,
			},
			JavaPromptAnswer::PickFolder => match chosen.folder() {
				Some(folder) => {
					let executable = resolve_java_executable(folder)?;
					self.register_checked(&executable, Some(major)).await
				}
				None => Err(JavaError::Cancelled),
			},
		}
	}

	#[tracing::instrument(level = "debug", skip(self, progress))]
	async fn download_and_register(
		&self,
		major: u32,
		progress: Option<&GroupedProgressSession>,
	) -> JavaResult<JavaRuntime> {
		for provider in vendors::runtime_providers() {
			let vendor = provider.vendor();

			let packages = provider.list_packages(Some(major), &self.net).await;
			let package = match &packages {
				Ok(packages) => match packages
					.iter()
					.find(|p| p.java_version.first() == Some(&major))
					.or_else(|| packages.first())
				{
					Some(package) => package,
					None => {
						tracing::warn!(?vendor, major, "no packages found");
						continue;
					}
				},
				Err(err) => {
					tracing::warn!(?vendor, major, "failed to query packages: {err}");
					continue;
				}
			};

			tracing::info!(?vendor, major, "downloading Java runtime");
			match provider
				.install_package(package, &self.net, &self.events, progress)
				.await
			{
				Ok(executable) => return self.register_checked(&executable, Some(major)).await,
				Err(err) => tracing::warn!(?vendor, major, "install failed: {err}"),
			}
		}

		Err(JavaError::PackageNotFound { major })
	}

	#[tracing::instrument(level = "debug", skip(self))]
	async fn register_checked(
		&self,
		executable: &Path,
		expected_major: Option<u32>,
	) -> JavaResult<JavaRuntime> {
		let info = checker::check_java_runtime(executable.display().to_string()).await?;
		let major = parse_major_version(&info.version)?;

		if let Some(expected) = expected_major
			&& major != expected
		{
			return Err(JavaError::VersionMismatch {
				expected,
				found: major,
			});
		}

		self.persist(executable, &info).await
	}

	/// Skips anything that fails to probe a broken JDK must not abort the scan
	#[tracing::instrument(level = "debug", skip_all)]
	async fn register_located(&self, located: &[(PathBuf, JavaCheckInfo)]) -> JavaResult<()> {
		for (path, info) in located {
			if let Err(err) = self.persist(path, info).await {
				tracing::warn!(
					path = %path.display(),
					version = %info.version,
					"skipping located Java runtime: {err}"
				);
			}
		}
		Ok(())
	}

	async fn persist(&self, executable: &Path, info: &JavaCheckInfo) -> JavaResult<JavaRuntime> {
		let runtime = JavaRuntime {
			absolute_path: executable.to_string_lossy().into_owned(),
			major: parse_major_version(&info.version)?,
			version: info.version.clone(),
			vendor: JavaVendor::from_str(&info.vendor)
				.unwrap_or_else(|_| JavaVendor::Other(info.vendor.clone())),
			os_arch: info.os_arch.clone(),
			is_jdk: info.is_jdk,
			probe_version: PROBE_VERSION,
		};

		Ok(self.store.upsert(&runtime).await?)
	}
}

fn provider_for_vendor(vendor: &JavaVendor) -> Option<Box<dyn vendors::JavaRuntimeProvider>> {
	vendors::runtime_providers()
		.into_iter()
		.find(|provider| &provider.vendor() == vendor)
}

/// Java 8 and earlier report `1.8.0_412` where the major is the *second*
/// component 9 and later report `21.0.3` where it is the first
pub(crate) fn parse_major_version(version: &str) -> Result<u32, JavaError> {
	if let Some(rest) = version.strip_prefix("1.") {
		let digit = rest.chars().next().ok_or_else(|| JavaError::ParseVersion {
			version: version.to_string(),
		})?;
		return digit.to_digit(10).ok_or_else(|| JavaError::ParseVersion {
			version: version.to_string(),
		});
	}

	let head = version.split('.').next().unwrap_or(version);
	head.parse::<u32>().map_err(|_| JavaError::ParseVersion {
		version: version.to_string(),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn legacy_versions_take_the_second_component() {
		assert_eq!(parse_major_version("1.8.0_412").unwrap(), 8);
		assert_eq!(parse_major_version("1.7.0").unwrap(), 7);
	}

	#[test]
	fn modern_versions_take_the_first() {
		assert_eq!(parse_major_version("21.0.3").unwrap(), 21);
		assert_eq!(parse_major_version("17").unwrap(), 17);
	}

	#[test]
	fn garbage_is_rejected_rather_than_defaulted() {
		assert!(parse_major_version("not-a-version").is_err());
		assert!(parse_major_version("1.x").is_err());
	}
}
