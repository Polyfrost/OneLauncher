use std::path::{Path, PathBuf};

use oneclient_net::ResponseNotifyOptions;
use crate::data::{
	java_executable_relative_path, JavaPackage, PackageArchive,
};
use oneclient_events::{GroupedProgressSession, TaskCategory, TaskPhase};
use oneclient_common::paths;
use oneclient_events::EventBus;
use oneclient_net::RequestClient;
use crate::error::JavaResult;

#[tracing::instrument(level = "debug", skip(net, events, progress))]
pub async fn install_package(
	package: &JavaPackage,
	net: &RequestClient,
	events: &EventBus,
	progress: Option<&GroupedProgressSession>,
) -> JavaResult<PathBuf> {
	let java_dir = paths::java_dir()?;
	polyio::create_dir_all(&java_dir).await?;

	let archive_path = java_dir.join(polyio::sanitize_path(&package.name));

	let major = package.java_version.first().copied().unwrap_or(0);

	// 0 means unknown the real total comes from Content-Length once headers land
	let expected_size = package.size.unwrap_or(0);
	let child = progress.map(|session| {
		session.child(
			format!("{} {}", package.vendor, major),
			expected_size,
			TaskCategory::Java,
		)
	});

	let notify = match &child {
		Some(child) => ResponseNotifyOptions::grouped(child.clone()),
		None => ResponseNotifyOptions::standalone(format!(
			"Installing {} {}",
			package.vendor, major
		))
		.done_label(format!("Installed {} {}", package.vendor, major)),
	};

	if package.checksum.is_none() {
		tracing::warn!(
			vendor = %package.vendor,
			major,
			"vendor published no usable checksum; installing runtime unverified"
		);
	}

	oneclient_net::download_verified(
		net,
		events,
		&package.download_url,
		&archive_path,
		package.checksum.as_ref(),
		expected_size,
		Some(notify),
	)
	.await?;

	let extract_root = java_dir.join(stem_without_archive(&package.name));

	if let Some(child) = &child {
		child.set_phase(TaskPhase::Extracting);
	}

	match package.archive {
		PackageArchive::Zip => polyio::extract_zip(&archive_path, &extract_root).await?,
		PackageArchive::TarGz => polyio::extract_tar_gz(&archive_path, &extract_root).await?,
	}

	let executable = resolve_installed_executable(&extract_root, package);

	#[cfg(unix)]
	{
		let _ = tokio::process::Command::new("chmod")
			.arg("755")
			.arg(&executable)
			.output()
			.await;
	}

	let _ = polyio::remove_file(&archive_path).await;

	tracing::info!(vendor = %package.vendor, major, "installed Java runtime");

	Ok(executable)
}

/// The directory this crate extracted for `executable`, or `None` when the
/// runtime sits outside the launcher's java dir which is where a folder the
/// user added themselves points
#[tracing::instrument(level = "debug")]
fn managed_install_root(executable: &Path) -> JavaResult<Option<PathBuf>> {
	let java_dir = paths::java_dir()?;

	let Ok(root) = polyio::canonicalize(&java_dir) else {
		return Ok(None);
	};

	// Refuses to resolve once the executable is gone which is the safe answer
	// there is no way left to prove what the path pointed at
	let canon = match polyio::ensure_under(executable, [&root]) {
		Ok(Some(canon)) => canon,
		Ok(None) => return Ok(None),
		Err(err) => {
			tracing::debug!("could not resolve the java runtime path: {err}");
			return Ok(None);
		}
	};

	let Ok(relative) = canon.strip_prefix(&root) else {
		return Ok(None);
	};

	match relative.components().next() {
		Some(std::path::Component::Normal(name)) => Ok(Some(root.join(name))),
		_ => Ok(None),
	}
}

/// Whether [`remove_installed_package`] would take this runtime's files with
/// it so the UI can say up front what removing it costs
#[must_use]
pub fn is_launcher_managed(executable: &Path) -> bool {
	managed_install_root(executable).is_ok_and(|root| root.is_some())
}

/// Only ever touches OneClient's own java dir a runtime the user added from
/// their own folder is left on disk untouched
#[tracing::instrument(level = "debug")]
pub async fn remove_installed_package(executable: &Path) -> JavaResult<bool> {
	let Some(install_root) = managed_install_root(executable)? else {
		tracing::debug!("java runtime is not launcher-managed leaving its files alone");
		return Ok(false);
	};

	polyio::remove_dir_all(&install_root).await?;

	tracing::info!(path = %install_root.display(), "removed installed Java runtime files");
	Ok(true)
}

#[tracing::instrument(level = "debug")]
fn resolve_installed_executable(extract_root: &Path, package: &JavaPackage) -> PathBuf {
    let mut base_path = extract_root.to_path_buf();

    if let Ok(entries) = std::fs::read_dir(extract_root) {
        let valid_dirs: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect();

        if valid_dirs.len() == 1 {
            base_path = valid_dirs[0].clone();
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = package;
        let subfolder_exec = base_path.join(java_executable_relative_path());
        if subfolder_exec.exists() {
            subfolder_exec
        } else {
            extract_root.join(java_executable_relative_path())
        }
    }

    #[cfg(target_os = "macos")]
    {
        use crate::vendors::JavaVendor;

        if !base_path.join("Contents").join("Home").exists()
            && let Ok(entries) = std::fs::read_dir(&base_path)
                && let Some(dir) = entries.flatten().find(|entry| {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy();
                    (name.ends_with(".jre") || name.ends_with(".jdk") || name.contains("zulu"))
                        && entry.path().join("Contents").join("Home").exists()
                }) {
                    base_path = dir.path();
                }

        if package.vendor == JavaVendor::Zulu
            && let Some(major) = package.java_version.first() {
                let zulu_bundle = base_path.join(format!("zulu-{major}.jre"));
                if zulu_bundle.join("Contents").join("Home").exists() {
                    base_path = zulu_bundle;
                }
            }

        base_path.join(java_executable_relative_path())
    }
}

fn stem_without_archive(name: &str) -> String {
	let path = std::path::Path::new(name);
	let stem = path
		.file_stem()
		.and_then(|s| s.to_str())
		.unwrap_or(name);

	if stem.ends_with(".tar") {
		std::path::Path::new(stem)
			.file_stem()
			.and_then(|s| s.to_str())
			.map(String::from)
			.unwrap_or_else(|| stem.to_string())
	} else {
		stem.to_string()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::{AtomicU32, Ordering};

	/// Keeps the whole suite inside a temp tree the override is a `OnceLock` so
	/// the first caller wins and every case works under one java dir
	fn java_dir() -> PathBuf {
		static ONCE: std::sync::Once = std::sync::Once::new();
		ONCE.call_once(|| {
			paths::set_launcher_dir(
				std::env::temp_dir().join(format!("oneclient-java-test-{}", std::process::id())),
			);
		});

		let dir = paths::java_dir().unwrap();
		std::fs::create_dir_all(&dir).unwrap();
		dir
	}

	fn install_runtime(root: &Path) -> (PathBuf, PathBuf) {
		static N: AtomicU32 = AtomicU32::new(0);
		let install = root.join(format!("zulu21-{}", N.fetch_add(1, Ordering::Relaxed)));
		let bin = install.join("zulu21.0.12").join("bin");
		std::fs::create_dir_all(&bin).unwrap();

		let executable = bin.join("java");
		std::fs::write(&executable, b"").unwrap();
		(install, executable)
	}

	#[test]
	fn resolves_the_extracted_directory_not_the_executable_parent() {
		let java_dir = java_dir();
		let (install, executable) = install_runtime(&java_dir);

		let resolved = managed_install_root(&executable).unwrap().unwrap();

		assert_eq!(resolved, polyio::canonicalize(&install).unwrap());
	}

	#[tokio::test]
	async fn removing_a_managed_runtime_deletes_its_files() {
		let java_dir = java_dir();
		let (install, executable) = install_runtime(&java_dir);

		assert!(remove_installed_package(&executable).await.unwrap());

		assert!(!install.exists());
		assert!(java_dir.exists(), "the java dir itself must survive");
	}

	#[tokio::test]
	async fn a_runtime_outside_the_java_dir_keeps_its_files() {
		let java_dir = java_dir();
		let elsewhere = java_dir.parent().unwrap().join("user-picked-jdk");
		let (_, executable) = install_runtime(&elsewhere);

		assert_eq!(managed_install_root(&executable).unwrap(), None);
		assert!(!remove_installed_package(&executable).await.unwrap());
		assert!(executable.exists(), "a folder the user added is not ours to delete");

		std::fs::remove_dir_all(&elsewhere).unwrap();
	}

	#[test]
	fn a_traversal_back_out_of_the_java_dir_is_not_managed() {
		let java_dir = java_dir();
		let elsewhere = java_dir.parent().unwrap().join("traversal-jdk");
		let (_, executable) = install_runtime(&elsewhere);

		let sneaky = java_dir
			.join("..")
			.join("traversal-jdk")
			.join(executable.strip_prefix(&elsewhere).unwrap());

		assert_eq!(managed_install_root(&sneaky).unwrap(), None);

		std::fs::remove_dir_all(&elsewhere).unwrap();
	}

	#[test]
	fn the_managed_flag_matches_what_removal_would_actually_delete() {
		let java_dir = java_dir();
		let (_, ours) = install_runtime(&java_dir);
		let elsewhere = java_dir.parent().unwrap().join("flagged-jdk");
		let (_, theirs) = install_runtime(&elsewhere);

		assert!(is_launcher_managed(&ours));
		assert!(!is_launcher_managed(&theirs));

		std::fs::remove_dir_all(&elsewhere).unwrap();
	}

	#[test]
	fn the_java_dir_itself_is_never_managed() {
		let java_dir = java_dir();

		assert_eq!(managed_install_root(&java_dir).unwrap(), None);
	}
}
