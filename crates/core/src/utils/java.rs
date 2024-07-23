//! **Java Utilities**
//!
//! Async utilities for managing and downloading Java versions.

use crate::State;
use futures::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::task::JoinError;

#[cfg(target_os = "windows")]
use winreg::{
	enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY},
	RegKey,
};

use super::io;

/// A struct representing a single version of the Java Runtime Environment
/// Use [`locate_java`] to get an array of all located java instances on the machine
/// paths from https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/java/JavaUtils.cpp under GPL 3.0
/// paths and java_check from https://github.com/modrinth/theseus/blob/master/theseus/src/util/jre.rs under MIT
#[cfg_attr(feature = "tauri", derive(specta::Type))]
#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct JavaVersion {
	pub version: String,
	pub arch: String,
	pub path: String,
}

/// JVM Entrypoint (WINDOWS ONLY)
#[cfg(target_os = "windows")]
#[tracing::instrument]
pub async fn locate_java() -> Result<Vec<JavaVersion>, JavaError> {
	let mut paths = HashSet::new();
	paths.extend(locate_java_path().await);
	paths.extend(locate_installed_java().await?);
	if let Ok(java_home) = env::var("JAVA_HOME") {
		paths.insert(PathBuf::from(java_home));
	}

	// i hate windows.com
	let common_java_paths = [
		r"C:/Program Files/Java",
		r"C:\Program Files\Eclipse Adoptium",
		r"C:/Program Files (x86)/Java",
		r"C:\Program Files (x86)/Eclipse Adoptium",
	];

	for path in common_java_paths {
		let Ok(java_subs) = std::fs::read_dir(path) else {
			continue;
		};
		for java_sub in java_subs.flatten() {
			let full_path = java_sub.path();
			paths.insert(full_path.join("bin"));
		}
	}

	let common_key_paths = [
		r"SOFTWARE\\JavaSoft\\Java Runtime Environment",
		r"SOFTWARE\\JavaSoft\\Java Development Kit",
		r"SOFTWARE\\JavaSoft\\JRE",
		r"SOFTWARE\\JavaSoft\\JDK",
		r"SOFTWARE\\AdoptOpenJDK\\JRE",
		r"SOFTWARE\\AdoptOpenJDK\\JDK",
		r"SOFTWARE\\Eclipse Foundation\\JDK",
		r"SOFTWARE\\Eclipse Foundation\\JRE",
		r"SOFTWARE\\Eclipse Adoptium\\JRE",
		r"SOFTWARE\\Eclipse Adoptium\\JDK",
		r"SOFTWARE\\Microsoft\\JDK",
		r"SOFTWARE\\Azul Systems\\Zulu",
		r"SOFTWARE\\BellSoft\\Liberica",
	];

	for key in key_paths {
		if let Ok(java_key) = RegKey::predef(HKEY_LOCAL_MACHINE)
			.open_subkey_with_flags(key, KEY_READ | KEY_WOW64_32KEY)
		{
			paths.extend(get_java_paths_from_windows(java_key));
		}
		if let Ok(java_key) = RegKey::predef(HKEY_LOCAL_MACHINE)
			.open_subkey_with_flags(key, KEY_READ | KEY_WOW64_64KEY)
		{
			paths.extend(get_java_paths_from_windows(java_key));
		}
	}

	let java = check_java(paths).await.into_iter().collect();

	Ok(java);
}

#[cfg(target_os = "windows")]
#[tracing::instrument]
pub fn get_java_paths_from_windows(java_key: RegKey) -> HashSet<PathBuf> {
	let mut paths = HashSet::new();

	for subkey in java_key.enum_keys().flatten() {
		if let Ok(subkey) = java_key.open_subkey(subkey) {
			let common_subkey_value_names = [r"JavaHome", r"InstallationPath", r"\\hotspot\\MSI"];
			for subkey_value in common_subkey_value_names {
				let path: Result<String, std::io::Error> = subkey.get_value(subkey_value);
				let Ok(path) = path else { continue };
				paths.insert(PathBuf::from(path).join("bin"));
			}
		}
	}

	paths
}

/// JVM Entrypoint (MacOS ONLY)
#[cfg(target_os = "macos")]
#[tracing::instrument]
pub async fn locate_java() -> Result<Vec<JavaVersion>, JavaError> {
	let mut paths = HashSet::new();
	paths.extend(locate_java_path().await);
	paths.extend(locate_installed_java().await?);

	let jvm_base = PathBuf::from("/Library/Java/JavaVirtualMachines/");
	if let Ok(dir) = std::fs::read_dir(jvm_base) {
		for entry in dir.flatten() {
			let entry = entry.path().join("Contents/Home/bin");
			paths.insert(entry);
		}
	}

	let common_java_paths = [
		r"/System/Library/Frameworks/JavaVM.framework/Versions/Current/Commands",
		r"/Applications/Xcode.app/Contents/Applications/Application Loader.app/Contents/MacOS/itms/java",
		r"/Library/Internet Plug-Ins/JavaAppletPlugin.plugin/Contents/Home",
	];

	for path in common_java_paths {
		paths.insert(PathBuf::from(path));
	}

	let java = check_java(paths).await.into_iter().collect();

	Ok(java)
}

/// JVM Entrypoint (LINUX ONLY)
#[cfg(target_os = "linux")]
#[tracing::instrument]
pub async fn locate_java() -> Result<Vec<JavaVersion>, JavaError> {
	let mut paths = HashSet::new();
	paths.extend(locate_java_path().await);
	paths.extend(locate_installed_java().await?);

	let common_java_paths = [
		r"/usr",
		r"/usr/java",
		r"/usr/lib/jvm",
		r"/usr/lib64/jvm",
		r"/usr/lib32/jvm",
		r"/opt/jdk",
		r"/opt/jdks",
		r"/app/jdk",
	];

	for path in common_java_paths {
		let path = PathBuf::from(path);
		paths.insert(PathBuf::from(&path).join("jre").join("bin"));
		paths.insert(PathBuf::from(&path).join("bin"));
		if let Ok(dir) = std::fs::read_dir(path) {
			for entry in dir.flatten() {
				let entry_path = entry.path();
				paths.insert(entry_path.join("jre").join("bin"));
				paths.insert(entry_path.join("bin"));
			}
		}
	}

	let java = check_java(paths).await.into_iter().collect();

	Ok(java)
}

/// Locate JRE's auto-installed by the launcher instance.
#[tracing::instrument]
#[onelauncher_debug::debugger]
async fn locate_installed_java() -> Result<HashSet<PathBuf>, JavaError> {
	Box::pin(async move {
		let state = State::get().await.map_err(|_| JavaError::MutexError)?;
		let mut paths = HashSet::new();
		let java_base_path = state.directories.java_dir().await;

		if java_base_path.is_dir() {
			if let Ok(dir) = std::fs::read_dir(java_base_path) {
				for entry in dir.flatten() {
					let path = entry.path().join("bin");
					if let Ok(contents) = io::read_to_string(path.clone()).await {
						let entry = entry.path().join(contents);
						paths.insert(entry);
					} else {
						#[cfg(not(target_os = "macos"))]
						{
							let path = path.join(JAVA_BIN);
							paths.insert(path);
						}
					}
				}
			}
		}

		Ok(paths)
	})
	.await
}

/// Locate all installed Java instances from the PATH env variable cross-device.
#[tracing::instrument]
async fn locate_java_path() -> HashSet<PathBuf> {
	let paths = env::var("PATH").map(|p| env::split_paths(&p).collect::<HashSet<_>>());
	paths.unwrap_or_else(|_| HashSet::new())
}

#[cfg(target_os = "windows")]
pub const JAVA_BIN: &str = "javaw.exe";

#[cfg(not(target_os = "windows"))]
pub const JAVA_BIN: &str = "java";

/// Verifies that each Java instance is valid from a HashSet of PathBufs
#[tracing::instrument]
pub async fn check_java(paths: HashSet<PathBuf>) -> HashSet<JavaVersion> {
	let java_instances = stream::iter(paths.into_iter())
		.map(|p: PathBuf| tokio::task::spawn(async move { check_java_instance(&p).await }))
		.buffer_unordered(64)
		.collect::<Vec<_>>()
		.await;

	java_instances
		.into_iter()
		.flat_map(|p| p.ok())
		.flatten()
		.collect()
}

/// Verifies that a java instance [`Path`] is valid.
/// java_check from https://github.com/modrinth/theseus/blob/master/theseus/library/JavaInfo.class under MIT
#[tracing::instrument]
#[onelauncher_debug::debugger]
pub async fn check_java_instance(path: &Path) -> Option<JavaVersion> {
	let Ok(path) = io::canonicalize(path) else {
		return None;
	};
	let java = if path.file_name()?.to_str()? != JAVA_BIN {
		path.join(JAVA_BIN)
	} else {
		path
	};
	if !java.exists() {
		return None;
	}

	let bytes = include_bytes!("../../wrapper/java/JavaInfo.class");
	let tempdir: PathBuf = tempfile::tempdir().ok()?.into_path();
	if !tempdir.exists() {
		return None;
	}

	let file_path = tempdir.join("JavaInfo.class");
	tracing::info!(
		"checking java with {}",
		file_path.to_string_lossy().to_string()
	);
	io::write(&file_path, bytes).await.ok()?;

	let output = Command::new(&java)
		.arg("-cp")
		.arg(file_path.parent().unwrap())
		.arg("JavaInfo")
		.env_remove("_JAVA_OPTIONS")
		.output()
		.ok()?;
	let stdout = String::from_utf8_lossy(&output.stdout);
	tracing::info!("{}", stdout);

	let mut java_version = None;
	let mut java_arch = None;

	for line in stdout.lines() {
		let mut parts = line.split('=');
		let key = parts.next().unwrap_or_default();
		let value = parts.next().unwrap_or_default();

		if key == "os.arch" {
			java_arch = Some(value);
		} else if key == "java.version" {
			java_version = Some(value);
		}
	}

	if let Some(arch) = java_arch {
		if let Some(version) = java_version {
			let path = java.to_string_lossy().to_string();
			return Some(JavaVersion {
				path,
				version: version.to_string(),
				arch: arch.to_string(),
			});
		}
	}

	None
}

/// Get a formatted java version.
pub fn get_java_version(version: &str) -> Result<(u32, u32), JavaError> {
	let mut split = version.split('.');
	let major_opt = split.next();
	let mut major;
	let mut minor = if let Some(minor) = split.next() {
		major = major_opt.unwrap_or("1").parse::<u32>()?;
		minor.parse::<u32>()?
	} else {
		major = 1;
		major_opt
			.ok_or_else(|| JavaError::InvalidVersion(version.to_string()))?
			.parse::<u32>()?
	};

	if major > 1 {
		minor = major;
		major = 1;
	}

	Ok((major, minor))
}

/// An error wrapper for everything that can go wrong while managing Java installations.
#[derive(thiserror::Error, Debug)]
pub enum JavaError {
	#[error("failed to fetch environment variable: {0}")]
	EnvError(#[from] env::VarError),

	#[error("error running java command: {0}")]
	IOError(#[from] std::io::Error),

	#[error("failed to parse integer: {0}")]
	ParseError(#[from] std::num::ParseIntError),

	#[error("tokio error: {0}")]
	JoinError(#[from] JoinError),

	#[error("couldn't get a valid java version from {0}")]
	InvalidVersion(String),

	#[error("failed to get launcher state mutex!")]
	MutexError,
}
