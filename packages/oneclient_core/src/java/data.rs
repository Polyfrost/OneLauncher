use std::path::PathBuf;
use std::str::FromStr;

use oneclient_db::models::JavaVersionRow;
use serde::{Deserialize, Serialize};

use crate::java::vendors::JavaVendor;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRuntime {
	pub absolute_path: String,
	pub major: u32,
	pub version: String,
	pub vendor: JavaVendor,
	pub os_arch: String,
}

impl JavaRuntime {
	pub fn from_row(row: JavaVersionRow) -> Self {
		Self {
			absolute_path: row.absolute_path,
			major: row.major as u32,
			version: row.version,
			vendor: JavaVendor::from_str(&row.vendor).unwrap_or(JavaVendor::Other(row.vendor)),
			os_arch: row.os_arch,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaPackage {
	pub download_url: String,
	pub name: String,
	pub java_version: Vec<u32>,
	pub vendor: JavaVendor,
	pub archive: PackageArchive,
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
		PathBuf::from("Contents/Home/bin").join(crate::constants::JAVA_BIN)
	}
	#[cfg(not(target_os = "macos"))]
	{
		PathBuf::from("bin").join(crate::constants::JAVA_BIN)
	}
}
