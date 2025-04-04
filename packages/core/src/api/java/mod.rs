use std::{collections::HashMap, path::PathBuf};

use serde::Serialize;

use crate::{api::ingress::{init_ingress, init_ingress_opt, send_ingress, send_ingress_opt}, error::LauncherResult, store::ingress::IngressType, utils::io};

pub mod dao;

#[derive(Debug, thiserror::Error)]
pub enum JavaError {
	#[error("failed to parse version")]
	ParseVersion(#[from] std::num::ParseIntError),
	#[error("failed to execute java command")]
	Execute(#[from] std::io::Error),
}

/// Returns the relative path to the Java executable
#[must_use]
pub fn get_java_bin() -> PathBuf {
	#[cfg(target_os = "macos")]
	{
		PathBuf::new()
			.join("Contents")
			.join("Home")
			.join("bin")
			.join("java")
	}
	#[cfg(target_os = "windows")]
	{
		PathBuf::new().join("bin").join("javaw.exe")
	}
	#[cfg(not(any(target_os = "macos", target_os = "windows")))]
	{
		PathBuf::new().join("bin").join("java")
	}
}

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Serialize)]
pub struct JavaInfo {
	pub os_arch: String,
	pub java_version: String,
	pub java_vendor: String,
}

const JAVA_INFO_CLASS: &[u8] = include_bytes!("../../../assets/java/JavaInfo.class");

/// Accepts a path to a java executable and returns the [`JavaInfo`]
pub async fn check_java_runtime(absolute_path: &PathBuf, with_ingress: bool) -> LauncherResult<JavaInfo> {
	let id = init_ingress_opt(with_ingress, IngressType::JavaCheck, "Checking JRE information", 100.0).await?;
	let id = id.as_ref();

	let dir = io::tempdir()?;
	let file = dir.path().join("JavaInfo.class");
	send_ingress_opt(id, 25.0).await?;

	io::write(&file, JAVA_INFO_CLASS).await?;
	send_ingress_opt(id, 25.0).await?;

	let java_info = tokio::process::Command::new(absolute_path)
		.arg("-cp")
		.arg(dir.path())
		.arg("JavaInfo")
		.env_remove("_JAVA_OPTIONS")
		.output()
		.await
		.map_err(JavaError::from)?;

	let java_info = String::from_utf8_lossy(&java_info.stdout);
	send_ingress_opt(id, 50.0).await?;

	let info = java_info.lines()
		.map(|line| {
			let mut parts = line.splitn(2, '=');
			let key = parts.next().unwrap_or("unknown");
			let value = parts.next().unwrap_or("unknown");

			(key.to_string(), value.to_string())
		})
		.collect::<HashMap<_, _>>();

	Ok(JavaInfo {
		os_arch: info.get("os.arch").cloned().unwrap_or_else(|| String::from("unknown")),
		java_version: info.get("java.version").cloned().unwrap_or_else(|| String::from("unknown")),
		java_vendor: info.get("java.vendor").cloned().unwrap_or_else(|| String::from("unknown")),
	})
}

/// Attempts to scan common paths for Java installations.
///
/// **Can be heavy on I/O, don't run often!**
pub async fn locate_java() -> LauncherResult<HashMap<PathBuf, JavaInfo>> {
	let id = init_ingress(IngressType::JavaLocate, "Locating java installations", 110.0).await?;

	let paths = internal_locate_java();
	let mut valid = HashMap::new();

	send_ingress(&id, 10.0).await?;
	let total = paths.len();
	let step_amount = 100.0 / total as f64;

	for path in paths {
		send_ingress(&id, step_amount).await?;

		let Ok(info) = check_java_runtime(&path, false).await else {
			tracing::warn!("Java installation at '{}' is not valid", path.display());
			continue;
		};

		valid.insert(path, info);
	}

	Ok(valid)
}

#[cfg(target_os = "windows")]
fn internal_locate_java() -> Vec<PathBuf> {
	let mut found = Vec::new();

	// TODO: Implement Registry scanning
	// TODO(windows): More paths for Java installations

	let paths = vec![
		r"C:/Program Files/Java",
		r"C:/Program Files/Eclipse Adoptium",
		r"C:/Program Files (x86)/Java",
		r"C:/Program Files (x86)/Eclipse Adoptium",
	];

	for path in paths {
		let path = PathBuf::from(path).join(get_java_bin());
		if path.exists() {
			found.push(path);
		}
	}

	found
}

#[cfg(target_os = "macos")]
fn internal_locate_java() -> Vec<PathBuf> {
	let mut found = Vec::new();

	// TODO(macos): More paths for Java installations

	let paths = vec![
		r"/System/Library/Frameworks/JavaVM.framework/Versions/Current/Commands",
		r"/Applications/Xcode.app/Contents/Applications/Application Loader.app/Contents/MacOS/itms/java",
		r"/Library/Internet Plug-Ins/JavaAppletPlugin.plugin/Contents/Home",
	];

	for path in paths {
		let path = PathBuf::from(path).join(get_java_bin());
		if path.exists() {
			found.push(path);
		}
	}

	found
}

#[cfg(target_os = "linux")]
fn internal_locate_java() -> Vec<PathBuf> {
	let mut found = Vec::new();

	let paths = vec![
		r"/usr/java",
		r"/usr/lib/jvm",
		r"/usr/lib64/jvm",
		r"/usr/lib32/jvm",
		r"/opt/jdk",
		r"/opt/jdks",
		r"/app/jdk",
		r"~/.jdks", // Intellij downloaded jdks
		r"~/.gradle/jdks", // Gradle downloaded jdks
		r"~/.sdkman/candidates/java", // SDKMAN downloaded jdks
	];

	for path in paths {
		let path = PathBuf::from(path.replace('~', std::env::var("HOME").unwrap_or_default().as_str()));

		if let Ok(children) = std::fs::read_dir(path) {
			for child in children.flatten() {
				let path = child.path().join(get_java_bin());
				if path.exists() {
					found.push(path);
				}
			}
		}

	}

	found
}