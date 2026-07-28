//! What machine we are running on, decided once.
//!
//! Each of the four vendors had its own `cfg_select!` pair working this out,
//! which is how they drifted: Adoptium says `mac` where the rest say `macos`,
//! Adoptium says `alpine-linux` where Liberica and Zulu say `linux-musl`, and
//! Adoptium alone spells 32-bit x86 `x32`.
//!
//! Detection is shared here. The *naming* stays with each vendor: that part is
//! not duplication, it is what each API actually expects, and flattening it
//! would only hide the differences.

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
	/// `musl` distinguishes Alpine-style builds, which several vendors publish
	/// under a separate name.
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
	/// The machine this build is running on.
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

	/// Pointer width as the vendors spell it. Liberica takes architecture and
	/// bitness as separate query parameters rather than one combined token.
	#[must_use]
	pub const fn bitness(self) -> &'static str {
		match self.arch {
			HostArch::X86 | HostArch::Arm => "32",
			HostArch::X86_64 | HostArch::Aarch64 => "64",
		}
	}

	/// Archive format the vendors ship for this platform.
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
