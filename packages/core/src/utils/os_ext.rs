use interpulse::api::minecraft::Os;

/// An extension of [`Os`] for utilities on converting the current system's operating system
/// into an [`Os`] structure for simple management.
pub trait OsExt {
	/// Get the [`Os`] of the current system
	fn native() -> Self;

	/// Get the [`Os`] of the current system along with it's arch from the `java_arch`.
	fn native_arch(java_arch: &str) -> Self;

	/// Gets the OS from an OS + Arch
	#[must_use]
	fn get_os(&self) -> Self;
}

impl OsExt for Os {
	fn native() -> Self {
		match std::env::consts::OS {
			"windows" => Self::Windows,
			"macos" => Self::Osx,
			"linux" => Self::Linux,
			_ => Self::Unknown,
		}
	}

	fn native_arch(java_arch: &str) -> Self {
		if std::env::consts::OS == "windows" {
			if java_arch == "aarch64" {
				Self::WindowsArm64
			} else {
				Self::Windows
			}
		} else if std::env::consts::OS == "linux" {
			if java_arch == "aarch64" {
				Self::LinuxArm64
			} else if java_arch == "arm" {
				Self::LinuxArm32
			} else {
				Self::Linux
			}
		} else if std::env::consts::OS == "macos" {
			if java_arch == "aarch64" {
				Self::OsxArm64
			} else {
				Self::Osx
			}
		} else {
			Self::Unknown
		}
	}

	fn get_os(&self) -> Self {
		match self {
			Self::OsxArm64 => Self::Osx,
			Self::LinuxArm32 | Self::LinuxArm64 => Self::Linux,
			Self::WindowsArm64 => Self::Windows,
			_ => self.clone(),
		}
	}
}
