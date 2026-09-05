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
	pub is_jdk: bool,
	pub probe_version: u32,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaPackage {
	pub download_url: String,
	pub name: String,
	pub java_version: Vec<u32>,
	pub vendor: JavaVendor,
	pub archive: PackageArchive,
	/// Algorithm varies per vendor
	/// `None` when vendor metadata omitted or malformed it
	/// installs unverified rather than failing
	pub checksum: Option<Checksum>,
	/// Bytes when the vendor publishes it
	pub size: Option<u64>,
}

impl JavaPackage {
	/// Drops malformed vendor checksums which would otherwise make every
	/// install fail as a corrupt download
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
