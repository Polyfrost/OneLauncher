use std::path::{Path, PathBuf};


use crate::constants::JAVA_BIN;
use crate::java::data::java_executable_relative_path;
use crate::java::{JavaError, JavaResult};

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_java_executable(selection: impl AsRef<Path>) -> JavaResult<PathBuf> {
	let selection = selection.as_ref();

	if selection.is_file() {
		return validate_executable(selection);
	}

	let relative = java_executable_relative_path();
	let candidates = [
		selection.join(&relative),
		selection.join(JAVA_BIN),
		#[cfg(target_os = "macos")]
		selection
			.join("Contents")
			.join("Home")
			.join(&relative),
	];

	for candidate in candidates {
		if candidate.is_file() {
			return validate_executable(&candidate);
		}
	}

	if let Ok(entries) = std::fs::read_dir(selection) {
		for entry in entries.flatten() {
			let path = entry.path();
			if !path.is_dir() {
				continue;
			}

			let nested = path.join(&relative);
			if nested.is_file() {
				return validate_executable(&nested);
			}
		}
	}

	tracing::warn!(path = %selection.display(), "no Java executable found in selection");

	Err(JavaError::InvalidJavaPath {
		path: selection.display().to_string(),
	})
}

fn validate_executable(path: &Path) -> JavaResult<PathBuf> {
	let file_name = path
		.file_name()
		.and_then(|name| name.to_str())
		.unwrap_or_default();

	if file_name != JAVA_BIN && file_name != "java.exe" && file_name != "javaw.exe" {
		return Err(JavaError::InvalidJavaPath {
			path: path.display().to_string(),
		});
	}

	Ok(path.to_path_buf())
}
