use std::path::{Path, PathBuf};

use reqwest::Method;
use tracing::instrument;
use url::Url;

use crate::http::{ResponseNotifyOptions, ResponseOptions};
use crate::java::data::{
	java_executable_relative_path, JavaPackage, PackageArchive,
};
use crate::paths;
use crate::state::LauncherServices;
use crate::LauncherResult;

#[instrument(skip(services))]
pub async fn install_package(
	package: &JavaPackage,
	services: &LauncherServices,
) -> LauncherResult<PathBuf> {
	let java_dir = paths::java_dir()?;
	polyio::create_dir_all(&java_dir).await?;

	let archive_path = java_dir.join(polyio::sanitize_path(&package.name));

	let major = package.java_version.first().copied().unwrap_or(0);
	services.requester
		.download_file(
			reqwest::Request::new(Method::GET, Url::parse(&package.download_url)?),
			&archive_path,
			ResponseOptions {
				notify: Some(
					ResponseNotifyOptions::standalone(format!(
						"Installing {} {}",
						package.vendor, major
					))
					.done_label(format!("Installed {} {}", package.vendor, major)),
				),
			},
			services,
		)
		.await?;

	let extract_root = java_dir.join(stem_without_archive(&package.name));

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

	let _ = tokio::fs::remove_file(&archive_path).await;

	Ok(executable)
}

#[instrument]
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
        use crate::java::vendors::JavaVendor;
        
        if !base_path.join("Contents").join("Home").exists() {
            if let Ok(mut entries) = std::fs::read_dir(&base_path) {
                if let Some(dir) = entries.flatten().find(|entry| {
                    let file_name = entry.file_name();
                    let name = file_name.to_string_lossy();
                    (name.ends_with(".jre") || name.ends_with(".jdk") || name.contains("zulu"))
                        && entry.path().join("Contents").join("Home").exists()
                }) {
                    base_path = dir.path();
                }
            }
        }

        if package.vendor == JavaVendor::Zulu {
            if let Some(major) = package.java_version.first() {
                let zulu_bundle = base_path.join(format!("zulu-{major}.jre"));
                if zulu_bundle.join("Contents").join("Home").exists() {
                    base_path = zulu_bundle;
                }
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
