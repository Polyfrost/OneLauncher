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
	//events.notify("Extracting package...").body(format!("{} {}", package.vendor, major)).send();

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
