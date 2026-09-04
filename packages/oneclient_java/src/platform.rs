//! Host detection is shared here but the *naming* stays per-vendor
//! each API spells the same platform differently (e.g. Adoptium `mac`/`alpine-linux`/`x32`)

use crate::data::PackageArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArch {
	X86,
	X86_64,
	Arm,
	Aarch64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
	Windows,
	MacOs,
	/// Several vendors publish musl builds under a separate name
	Linux {
		musl: bool,
	},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostTarget {
	pub arch: HostArch,
	pub os: HostOs,
}

impl HostTarget {
	pub const CURRENT: Self = Self {
		arch: cfg_select! {
			target_arch = "x86" => HostArch::X86,
			target_arch = "x86_64" => HostArch::X86_64,
			target_arch = "arm" => HostArch::Arm,
			target_arch = "aarch64" => HostArch::Aarch64,
		},
		os: cfg_select! {
			target_os = "windows" => HostOs::Windows,
			target_os = "macos" => HostOs::MacOs,
			target_os = "linux" => cfg_select! {
				target_env = "musl" => HostOs::Linux { musl: true },
				_ => HostOs::Linux { musl: false },
			},
		},
	};

	/// Liberica takes architecture and bitness as separate query parameters
	#[must_use]
	pub const fn bitness(self) -> &'static str {
		match self.arch {
			HostArch::X86 | HostArch::Arm => "32",
			HostArch::X86_64 | HostArch::Aarch64 => "64",
		}
	}

	#[must_use]
	pub const fn archive(self) -> PackageArchive {
		match self.os {
			HostOs::Windows => PackageArchive::Zip,
			_ => PackageArchive::TarGz,
		}
	}

	#[must_use]
	pub const fn archive_ext(self) -> &'static str {
		match self.os {
			HostOs::Windows => "zip",
			_ => "tar.gz",
		}
	}

	#[must_use]
	pub const fn is_musl(self) -> bool {
		matches!(self.os, HostOs::Linux { musl: true })
	}
}

#[cfg(windows)]
const GPU_PREFERENCES_KEY: &str = r"HKCU\Software\Microsoft\DirectX\UserGpuPreferences";

/// A JVM is started through either stub so both are registered
#[cfg(windows)]
const GPU_PREFERENCE_EXECUTABLES: [&str; 2] = ["javaw.exe", "java.exe"];

/// Absolute so a `reg.exe` earlier in the search order cannot run in its place
#[cfg(windows)]
fn reg_command() -> tokio::process::Command {
	const NO_WINDOW: u32 = 0x0800_0000;

	let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
	let exe = std::path::Path::new(&system_root)
		.join("System32")
		.join("reg.exe");

	let mut command = tokio::process::Command::new(exe);
	command.creation_flags(NO_WINDOW);
	command
}

#[cfg(windows)]
fn registered_value<'a>(query: &'a str, name: &str) -> Option<&'a str> {
	query.lines().find_map(|line| {
		let (value_name, data) = line.split_once("REG_SZ")?;
		if value_name.trim().eq_ignore_ascii_case(name) {
			Some(data.trim())
		} else {
			None
		}
	})
}

/// Windows writes `GpuPreference=0;` ("let Windows decide") for any app added
/// through Settings, so only 1 or 2 is a choice worth leaving alone
#[cfg(windows)]
fn has_explicit_preference(data: &str) -> bool {
	data.split(';')
		.find_map(|part| part.trim().strip_prefix("GpuPreference="))
		.is_some_and(|preference| matches!(preference.trim(), "1" | "2"))
}

#[cfg(windows)]
pub async fn prefer_dedicated_gpu(java_path: &std::path::Path) {
	let Some(dir) = java_path.parent() else { return };

	let mut targets = Vec::new();
	for exe in GPU_PREFERENCE_EXECUTABLES {
		let path = dir.join(exe);
		if polyio::try_exists(&path).await.unwrap_or(false) {
			targets.push(path);
		}
	}

	if targets.is_empty() {
		return;
	}

	// One query for the whole key instead of one per executable
	let query = match reg_command()
		.args(["query", GPU_PREFERENCES_KEY])
		.output()
		.await
	{
		Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
		Err(err) => {
			tracing::warn!(%err, "could not run reg.exe");
			return;
		}
	};

	for path in targets {
		if registered_value(&query, &path.to_string_lossy()).is_some_and(has_explicit_preference) {
			continue;
		}

		let added = reg_command()
			.args(["add", GPU_PREFERENCES_KEY, "/v"])
			.arg(&path)
			.args(["/t", "REG_SZ", "/d", "GpuPreference=2;", "/f"])
			.output()
			.await;

		match added {
			Ok(out) if out.status.success() => {
				tracing::info!(path = %path.display(), "registered JVM for high-performance GPU");
			}
			Ok(out) => tracing::warn!(
				path = %path.display(),
				stderr = %String::from_utf8_lossy(&out.stderr).trim(),
				"could not set GPU preference"
			),
			Err(err) => tracing::warn!(%err, "could not run reg.exe"),
		}
	}
}

/// Runtimes install into version-stamped folders so without this the key keeps
/// a dead entry for every upgrade
#[cfg(windows)]
pub async fn forget_dedicated_gpu(java_path: &std::path::Path) {
	let Some(dir) = java_path.parent() else { return };

	for exe in GPU_PREFERENCE_EXECUTABLES {
		let path = dir.join(exe);
		let removed = reg_command()
			.args(["delete", GPU_PREFERENCES_KEY, "/v"])
			.arg(&path)
			.arg("/f")
			.output()
			.await;

		match removed {
			Ok(out) if out.status.success() => {
				tracing::info!(path = %path.display(), "removed GPU preference");
			}
			// A value that was never written is reported as an error
			Ok(_) => {}
			Err(err) => tracing::warn!(%err, "could not run reg.exe"),
		}
	}
}

#[cfg(not(windows))]
pub async fn prefer_dedicated_gpu(_java_path: &std::path::Path) {}

#[cfg(not(windows))]
pub async fn forget_dedicated_gpu(_java_path: &std::path::Path) {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn bitness_matches_the_architecture() {
		assert_eq!(HostTarget::CURRENT.bitness(), {
			if cfg!(target_pointer_width = "64") {
				"64"
			} else {
				"32"
			}
		});
	}

	#[test]
	fn windows_ships_zips_and_everything_else_tarballs() {
		let expected = if cfg!(windows) {
			PackageArchive::Zip
		} else {
			PackageArchive::TarGz
		};
		assert_eq!(HostTarget::CURRENT.archive(), expected);
	}

	#[test]
	fn musl_is_only_ever_reported_on_linux() {
		if HostTarget::CURRENT.is_musl() {
			assert!(matches!(HostTarget::CURRENT.os, HostOs::Linux { .. }));
		}
	}
}
