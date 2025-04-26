use std::{fmt::Display, ops::Deref, str::FromStr};

use sea_orm::{sea_query::Nullable, DeriveValueType};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, DeriveValueType)]
pub struct Icon(String);

impl Nullable for Icon {
	fn null() -> sea_orm::Value {
		sea_orm::Value::String(None)
	}
}

impl Icon {
	pub fn from_hash(hash: String) -> Self {
		Self(format!("cache://{hash}"))
	}

	pub fn try_from_path(path: impl AsRef<std::path::Path>) -> Option<Self> {
		let path = path.as_ref();
		if !path.is_absolute() {
			return None;
		}

		Some(Self(format!("file://{}", path.display())))
	}

	pub fn try_from_url(url: url::Url) -> Option<Self> {
		if url.scheme() != "http" && url.scheme() != "https" {
			return None;
		}

		Some(Self(url.as_str().to_string()))
	}

	pub fn get_type(&self) -> IconType {
		if self.is_path() {
			IconType::Path
		} else if self.is_url() {
			IconType::Url
		} else if self.is_cached() {
			IconType::Cache
		} else {
			IconType::Unknown
		}
	}

	/// Returns true if the icon is a path to a file.
	pub fn is_path(&self) -> bool {
		self.0.starts_with("file://")
	}

	/// Returns true if the icon is a HTTP URL.
	pub fn is_url(&self) -> bool {
		self.0.starts_with("http://") || self.0.starts_with("https://")
	}

	/// Returns true if the icon is a cached icon. Relative to the cache directory.
	pub fn is_cached(&self) -> bool {
		self.0.starts_with("cache://")
	}
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconType {
	#[default]
	Unknown,
	Path,
	Url,
	Cache,
}

impl Display for IconType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl Deref for Icon {
	type Target = String;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl From<String> for Icon {
	fn from(s: String) -> Self {
		Self(s)
	}
}

impl FromStr for Icon {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self(s.to_string()))
	}
}

impl Display for Icon {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Icon(IconType::{}, \"{}\")", self.get_type(), self.0)
	}
}
