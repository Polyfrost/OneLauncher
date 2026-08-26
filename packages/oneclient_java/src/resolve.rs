use std::path::{Path, PathBuf};


use oneclient_common::constants::JAVA_BIN;
use crate::data::java_executable_relative_path;
use crate::error::{JavaError, JavaResult};

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

	Ok(prefer_javaw(path))
}

#[must_use]
pub fn prefer_javaw(path: impl AsRef<Path>) -> PathBuf {
	let path = path.as_ref();

	#[cfg(windows)]
	if path.file_name().is_some_and(|name| name == "java.exe") {
		let javaw = path.with_file_name("javaw.exe");
		if javaw.is_file() {
			return javaw;
		}
	}

	path.to_path_buf()
}

#[cfg(test)]
mod tests {
	use super::prefer_javaw;

	#[test]
	fn prefers_the_windowless_twin() {
		let dir = std::env::temp_dir().join("oneclient-prefer-javaw");
		std::fs::create_dir_all(&dir).unwrap();
		std::fs::write(dir.join("java.exe"), b"").unwrap();
		std::fs::write(dir.join("javaw.exe"), b"").unwrap();

		let picked = prefer_javaw(dir.join("java.exe"));
		let expected = if cfg!(windows) { "javaw.exe" } else { "java.exe" };
		assert_eq!(picked.file_name().unwrap(), expected);

		// A lone `java.exe` has nothing to swap to
		std::fs::remove_file(dir.join("javaw.exe")).unwrap();
		assert_eq!(
			prefer_javaw(dir.join("java.exe")).file_name().unwrap(),
			"java.exe"
		);

		std::fs::remove_dir_all(&dir).unwrap();
	}
}