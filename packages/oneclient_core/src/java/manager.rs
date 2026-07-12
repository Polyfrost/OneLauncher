use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oneclient_db::{dao, DbPool};

use crate::java::checker::JavaCheckInfo;
use crate::java::data::{JavaPackage, JavaRuntime};
use crate::java::resolve::resolve_java_executable;
use crate::java::vendors::{self, JavaVendor};
use crate::java::{self, JavaError};
use crate::notification::UserChoice;
use crate::state::LauncherState;
use crate::{LauncherResult, LauncherServices};

pub const INSTALLABLE_MAJORS: &[u32] = &[8, 11, 16, 17, 18, 19, 20, 21, 22, 23];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableJava {
	pub major: u32,
	pub package: JavaPackage,
}

pub struct JavaManager;

impl JavaManager {
	pub async fn prepare_java(
		state: &Arc<LauncherState>,
		major: u32,
		search_system: bool,
	) -> LauncherResult<JavaRuntime> {
		Self::prepare_java_with_services(&state.services, major, search_system, false).await
	}

	pub async fn prepare_java_auto(
		state: &Arc<LauncherState>,
		major: u32,
		search_system: bool,
	) -> LauncherResult<JavaRuntime> {
		Self::prepare_java_with_services(&state.services, major, search_system, true).await
	}

	pub async fn prepare_java_with_services(
		services: &LauncherServices,
		major: u32,
		search_system: bool,
		auto_install: bool,
	) -> LauncherResult<JavaRuntime> {
		if let Some(runtime) = get_latest_runtime(&services.db, major).await? {
			return Ok(runtime);
		}

		if search_system {
			register_located_runtimes(&services.db, java::locate::locate_java().await?).await?;

			if let Some(runtime) = get_latest_runtime(&services.db, major).await? {
				return Ok(runtime);
			}
		}

		if auto_install {
			return download_and_register(major, services).await;
		}

		match services.notifier.prompt_java_install(major).await? {
			UserChoice::Accept => download_and_register(major, services).await,
			UserChoice::Folder(folder) => register_custom_java(&services.db, folder, major).await,
			UserChoice::Cancel => Err(JavaError::Cancelled.into()),
		}
	}

	pub async fn list_runtimes(pool: &DbPool) -> LauncherResult<Vec<JavaRuntime>> {
		let rows = dao::java::list_all(pool).await?;
		Ok(rows.into_iter().map(JavaRuntime::from_row).collect())
	}

	pub async fn rescan(pool: &DbPool) -> LauncherResult<()> {
		register_located_runtimes(pool, java::locate::locate_java().await?).await
	}

	pub async fn install_runtime(
		services: &LauncherServices,
		major: u32,
	) -> LauncherResult<JavaRuntime> {
		download_and_register(major, services).await
	}

	pub async fn available_versions(
		services: &LauncherServices,
		vendor: &JavaVendor,
	) -> LauncherResult<Vec<AvailableJava>> {
		let Some(provider) = provider_for_vendor(vendor) else {
			return Ok(Vec::new());
		};

		let packages = provider.list_packages(None, services).await?;

		let mut by_major =
			BTreeMap::<u32, JavaPackage>::new();

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

		available.sort_by_key(|b| std::cmp::Reverse(b.major));

		Ok(available)
	}

	pub async fn install_runtime_from(
		services: &LauncherServices,
		vendor: &JavaVendor,
		major: u32,
	) -> LauncherResult<JavaRuntime> {
		let provider =
			provider_for_vendor(vendor).ok_or(JavaError::PackageNotFound { major })?;
		let package = provider
			.latest_package_by_major(major, services)
			.await?
			.ok_or(JavaError::PackageNotFound { major })?;
		let executable = provider.install_package(&package, services).await?;
		register_checked_java(&services.db, &executable, Some(major)).await
	}

	pub async fn add_custom_runtime(
		pool: &DbPool,
		folder: PathBuf,
	) -> LauncherResult<JavaRuntime> {
		let executable = resolve_java_executable(&folder)?;
		register_checked_java(pool, &executable, None).await
	}

	pub async fn remove_runtime(pool: &DbPool, absolute_path: &str) -> LauncherResult<()> {
		dao::java::delete_by_path(pool, absolute_path).await?;
		Ok(())
	}

	pub async fn java_for_profile(
		pool: &DbPool,
		java_path: Option<&str>,
	) -> LauncherResult<Option<JavaRuntime>> {
		let Some(path) = java_path else {
			return Ok(None);
		};

		let Some(row) = dao::java::get_by_path(pool, path).await? else {
			return Ok(None);
		};

		Ok(Some(JavaRuntime::from_row(row)))
	}
}

fn provider_for_vendor(vendor: &JavaVendor) -> Option<Box<dyn vendors::JavaRuntimeProvider>> {
	vendors::runtime_providers()
		.into_iter()
		.find(|provider| &provider.vendor() == vendor)
}

async fn download_and_register(
	major: u32,
	services: &LauncherServices,
) -> LauncherResult<JavaRuntime> {
	for provider in vendors::runtime_providers() {
		let vendor = provider.vendor();

        let packages = provider.list_packages(Some(major), services).await;

		let package = match &packages {
			Ok(packages) => match packages.iter().find(|p| p.java_version.first() == Some(&major)).or_else(|| packages.first()) {
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

		match provider.install_package(package, services).await {
			Ok(executable) => {
				return register_checked_java(&services.db, &executable, Some(major)).await;
			}
			Err(err) => {
				tracing::warn!(?vendor, major, "install failed: {err}");
			}
		}
	}

	Err(JavaError::PackageNotFound { major }.into())
}

async fn register_custom_java(
	pool: &DbPool,
	selection: PathBuf,
	expected_major: u32,
) -> LauncherResult<JavaRuntime> {
	let executable = resolve_java_executable(&selection)?;
	register_checked_java(pool, &executable, Some(expected_major)).await
}

async fn register_checked_java(
	pool: &DbPool,
	executable: &Path,
	expected_major: Option<u32>,
) -> LauncherResult<JavaRuntime> {
	let info = java::check_java_runtime(executable.display().to_string()).await?;
	let major = parse_major_version(&info.version)?;

	if let Some(expected) = expected_major
		&& major != expected
	{
		return Err(JavaError::VersionMismatch {
			expected,
			found: major,
		}
		.into());
	}

	persist_runtime(pool, executable, &info).await
}

async fn get_latest_runtime(pool: &DbPool, major: u32) -> LauncherResult<Option<JavaRuntime>> {
	let row = dao::java::get_latest_by_major(pool, major).await?;
	Ok(row.map(JavaRuntime::from_row))
}

async fn register_located_runtimes(
	pool: &DbPool,
	located: impl IntoIterator<Item = (PathBuf, JavaCheckInfo)>,
) -> LauncherResult<()> {
	for (path, info) in located {
		let _ = persist_runtime(pool, &path, &info).await?;
	}
	Ok(())
}

async fn persist_runtime(
	pool: &DbPool,
	executable: &Path,
	info: &JavaCheckInfo,
) -> LauncherResult<JavaRuntime> {
	let major = parse_major_version(&info.version)?;
	let row = dao::java::insert(
		pool,
		&executable.to_string_lossy(),
		major,
		&info.version,
		&info.vendor,
		&info.os_arch,
	)
	.await?;

	Ok(JavaRuntime::from_row(row))
}

fn parse_major_version(version: &str) -> Result<u32, JavaError> {
	if let Some(rest) = version.strip_prefix("1.") {
		let digit = rest.chars().next().ok_or_else(|| JavaError::ParseVersion {
			version: version.to_string(),
		})?;
		return digit
			.to_digit(10)
			.ok_or_else(|| JavaError::ParseVersion {
				version: version.to_string(),
			});
	}

	let head = version.split('.').next().unwrap_or(version);
	head.parse::<u32>()
		.map_err(|_| JavaError::ParseVersion {
			version: version.to_string(),
		})
}
