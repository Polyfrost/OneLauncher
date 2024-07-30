//! **Platform Utilities**
//!
//! Async utilities for managing OS-specific [`interpulse`] operations and rules.

use interpulse::api::minecraft::{Os, OsRule};
use regex::Regex;

// todo: add this to interpulse (maybe, generally we keep it unimplemented but these are pretty basic functions).

/// An extension of [`Os`] for utilities on converting the current system's operating system
/// into an [`Os`] structure for simple management.
pub trait OsExt {
	/// Get the [`Os`] of the current system
	fn native() -> Self;

	/// Get the [`Os`] of the current system along with it's arch from the `java_arch`.
	fn native_arch(java_arch: &str) -> Self;
}

impl OsExt for Os {
	fn native_arch(java_arch: &str) -> Self {
		if std::env::consts::OS == "windows" {
			if java_arch == "aarch64" {
				Os::WindowsArm64
			} else {
				Os::Windows
			}
		} else if std::env::consts::OS == "linux" {
			if java_arch == "aarch64" {
				Os::LinuxArm64
			} else if java_arch == "arm" {
				Os::LinuxArm32
			} else {
				Os::Linux
			}
		} else if std::env::consts::OS == "macos" {
			if java_arch == "aarch64" {
				Os::OsxArm64
			} else {
				Os::Osx
			}
		} else {
			Os::Unknown
		}
	}

	fn native() -> Self {
		match std::env::consts::OS {
			"windows" => Self::Windows,
			"macos" => Self::Osx,
			"linux" => Self::Linux,
			_ => Self::Unknown,
		}
	}
}

/// Handles an os-specific [`OsRule`], returning if the [`Os`] matches the rule.
pub fn os_rule(rule: &OsRule, java_arch: &str, updated: bool) -> bool {
	let mut rule_match = true;

	if let Some(ref arch) = rule.arch {
		rule_match &= !matches!(arch.as_str(), "x86" | "arm");
	}

	if let Some(name) = &rule.name {
		if updated && (name != &Os::LinuxArm64 || name != &Os::LinuxArm32) {
			rule_match &= &Os::native() == name || &Os::native_arch(java_arch) == name;
		} else {
			rule_match &= &Os::native_arch(java_arch) == name;
		}
	}

	if let Some(version) = &rule.version {
		if let Ok(regex) = Regex::new(version.as_str()) {
			rule_match &= regex.is_match(&sys_info::os_release().unwrap_or_default());
		}
	}

	rule_match
}

/// Gets the seperator between Java classpaths in metadata (`";"` on Windows, `";"` on others).
pub fn classpath_separator(java_arch: &str) -> &'static str {
	match Os::native_arch(java_arch) {
		Os::Osx | Os::OsxArm64 | Os::Linux | Os::LinuxArm32 | Os::LinuxArm64 | Os::Unknown => ":",
		Os::Windows | Os::WindowsArm64 => ";",
	}
}
