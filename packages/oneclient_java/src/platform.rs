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

#[cfg(windows)]
pub async fn prefer_dedicated_gpu(java_path: &std::path::Path) {
	use std::os::windows::process::CommandExt;
	const NO_WINDOW: u32 = 0x0800_0000;
	const KEY: &str = r"HKCU\Software\Microsoft\DirectX\UserGpuPreferences";

	let Some(dir) = java_path.parent() else { return };

	for exe in ["javaw.exe", "java.exe"] {
		let path = dir.join(exe);
		if !path.exists() {
			continue;
		}

		let existing = tokio::process::Command::new("reg")
			.args(["query", KEY, "/v"])
			.arg(&path)
			.creation_flags(NO_WINDOW)
			.output()
			.await;
		if existing.map(|out| out.status.success()).unwrap_or(false) {
			continue;
		}

		let added = tokio::process::Command::new("reg")
			.args(["add", KEY, "/v"])
			.arg(&path)
			.args(["/t", "REG_SZ", "/d", "GpuPreference=2;", "/f"])
			.creation_flags(NO_WINDOW)
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

#[cfg(not(windows))]
pub async fn prefer_dedicated_gpu(_java_path: &std::path::Path) {}
