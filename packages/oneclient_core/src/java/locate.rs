use std::collections::HashMap;
use std::path::PathBuf;


use crate::constants::JAVA_BIN;
use crate::java::checker::{check_java_runtime, JavaCheckInfo};
use crate::java::data::java_executable_relative_path;
use crate::LauncherResult;

#[tracing::instrument(level = "debug")]
pub async fn locate_java() -> LauncherResult<HashMap<PathBuf, JavaCheckInfo>> {
	let paths = internal_locate_java();
	let mut valid = HashMap::new();

	for path in paths {
		let Ok(info) = check_java_runtime(path.display().to_string()).await else {
			tracing::warn!("java installation at '{}' is not valid", path.display());
			continue;
		};
		valid.insert(path, info);
	}

	tracing::debug!(count = valid.len(), "located Java runtimes");

	Ok(valid)
}

#[cfg(target_os = "windows")]
#[tracing::instrument(level = "debug")]
fn internal_locate_java() -> Vec<PathBuf> {
	let mut java_homes = Vec::new();

    let windows_paths = {
        let mut paths = vec![
            PathBuf::from(r"C:\Program Files\Java"),
            PathBuf::from(r"C:\Program Files (x86)\Java"),
            PathBuf::from(r"C:\Program Files\Eclipse Adoptium"),
            PathBuf::from(r"C:\Program Files\Amazon Corretto"),
            PathBuf::from(r"C:\Program Files\Zulu"),
            PathBuf::from(r"C:\Program Files\OpenJDK"),
            PathBuf::from(r"C:\Program Files\Microsoft"),
            PathBuf::from(r"C:\ProgramData\chocolatey\lib"),
        ];
        if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            paths.push(profile.join(".jdks"));
            paths.push(profile.join(".gradle").join("jdks"));
            paths.push(profile.join(".m2").join("wrapper").join("dists"));
            paths.push(profile.join("scoop").join("apps"));
        }
        paths
    };

	for base_path in windows_paths {
		if let Ok(entries) = std::fs::read_dir(base_path) {
			for entry in entries.flatten() {
				if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
					let java_exe = entry.path().join(java_executable_relative_path());
					if java_exe.exists() {
						java_homes.push(java_exe);
					}
				}
			}
		}
	}

	if let Ok(java_home) = std::env::var("JAVA_HOME") {
		let java_exe = PathBuf::from(java_home).join(java_executable_relative_path());
		if java_exe.exists() {
			java_homes.push(java_exe);
		}
	}

	if let Ok(path) = std::env::var("PATH") {
		for path_entry in path.split(';') {
			let java_exe = PathBuf::from(path_entry).join(JAVA_BIN);
			if java_exe.exists() && !java_homes.contains(&java_exe) {
				java_homes.push(java_exe);
			}
		}
	}

	java_homes
}

#[cfg(target_os = "macos")]
#[tracing::instrument(level = "debug")]
fn internal_locate_java() -> Vec<PathBuf> {
	let mut found = Vec::new();

    let macos_paths = {
        let mut paths = vec![
            PathBuf::from("/Library/Java/JavaVirtualMachines"),
            PathBuf::from("/System/Library/Java/JavaVirtualMachines"),
            PathBuf::from("/System/Library/Frameworks/JavaVM.framework/Versions/Current/Commands"),
            PathBuf::from("/Library/Internet Plug-Ins/JavaAppletPlugin.plugin/Contents/Home"),
            PathBuf::from("/opt/homebrew/Cellar"),
            PathBuf::from("/opt/homebrew/opt"),
            PathBuf::from("/usr/local/Cellar"),
            PathBuf::from("/usr/local/opt"),
        ];
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            paths.push(home.join("Library").join("Java").join("JavaVirtualMachines"));
            paths.push(home.join(".jdks"));
            paths.push(home.join(".sdkman").join("candidates").join("java"));
        }
        paths
    };

	for path in macos_paths {
		if let Ok(children) = std::fs::read_dir(path) {
			for child in children.flatten() {
				let java_exe = child.path().join(java_executable_relative_path());
				if java_exe.exists() {
					found.push(java_exe);
				}
			}
		}
	}

	find_java_in_path(found)
}

#[cfg(target_os = "linux")]
#[tracing::instrument(level = "debug")]
fn internal_locate_java() -> Vec<PathBuf> {
	let mut found = Vec::new();

    let linux_paths = {
        let mut paths = vec![
            PathBuf::from("/usr/lib/jvm"),
            PathBuf::from("/usr/lib64/jvm"),
            PathBuf::from("/usr/lib32/jvm"),
            PathBuf::from("/usr/java"),
            PathBuf::from("/opt/jdk"),
            PathBuf::from("/opt/jdks"),
            PathBuf::from("/app/jdk"),
        ];
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            paths.push(home.join(".jdks"));
            paths.push(home.join(".gradle").join("jdks"));
            paths.push(home.join(".sdkman").join("candidates").join("java"));
        }
        paths
    };

	for path in linux_paths {
		if let Ok(children) = std::fs::read_dir(path) {
			for child in children.flatten() {
				let java_exe = child.path().join(java_executable_relative_path());
				if java_exe.exists() {
					found.push(java_exe);
				}
			}
		}
	}

	find_java_in_path(found)
}

#[cfg(unix)]
fn find_java_in_path(mut found: Vec<PathBuf>) -> Vec<PathBuf> {
	if let Ok(path) = std::env::var("PATH") {
		for path_entry in path.split(':') {
			let java = PathBuf::from(path_entry).join(JAVA_BIN);
			if java.exists() && java.is_file() && !found.contains(&java) {
				found.push(java);
			}
		}
	}
	found
}
