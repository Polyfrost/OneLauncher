use std::path::PathBuf;

use polyio::Checksum;
use serde::{Deserialize, Serialize};

use crate::vendors::JavaVendor;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRuntime {
	pub absolute_path: String,
	pub major: u32,
	pub version: String,
	pub vendor: JavaVendor,
	pub os_arch: String,
	/// Whether this is a development kit rather than a bare runtime image, as
	/// the probe reported it when the runtime was recorded (see
	/// [`crate::JavaCheckInfo::is_jdk`]).
	pub is_jdk: bool,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaPackage {
	pub download_url: String,
	pub name: String,
	pub java_version: Vec<u32>,
	pub vendor: JavaVendor,
	pub archive: PackageArchive,
	/// What the vendor promises the archive hashes to. Every vendor publishes
	/// one, in its own algorithm — SHA-256 for Adoptium, Corretto and Zulu,
	/// SHA-1 for Liberica — so this carries the algorithm with it.
	///
	/// `None` only when a vendor's metadata omitted or malformed it. A runtime
	/// that cannot be verified is still worth installing (the alternative is a
	/// launcher that cannot start the game at all), so this degrades to an
	/// unverified download rather than a hard failure.
	pub checksum: Option<Checksum>,
	/// Archive size in bytes when the vendor publishes it, for progress bars
	/// that would otherwise wait on `Content-Length`.
	pub size: Option<u64>,
}

impl JavaPackage {
	/// Drops a checksum the vendor returned in an unusable shape.
	///
	/// A vendor that starts sending an empty string or a placeholder would
	/// otherwise make every runtime install fail with what looks to the user
	/// like a corrupt download, three retries deep.
	#[must_use]
	pub fn with_checksum(mut self, checksum: Option<Checksum>) -> Self {
		self.checksum = checksum.filter(|sum| {
			let ok = sum.is_well_formed();
			if !ok {
				tracing::warn!(
					algorithm = sum.algorithm.name(),
					hex = %sum.hex,
					"vendor published a malformed checksum; installing unverified"
				);
			}
			ok
		});
		self
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageArchive {
	Zip,
	TarGz,
}

impl PackageArchive {
	pub fn from_filename(name: &str) -> Self {
		if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
			Self::TarGz
		} else {
			Self::Zip
		}
	}
}

pub fn java_executable_relative_path() -> PathBuf {
	#[cfg(target_os = "macos")]
	{
		PathBuf::from("Contents/Home/bin").join(oneclient_common::constants::JAVA_BIN)
	}
	#[cfg(not(target_os = "macos"))]
	{
		PathBuf::from("bin").join(oneclient_common::constants::JAVA_BIN)
	}
}
