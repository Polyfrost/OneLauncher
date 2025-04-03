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
	pub fn new(icon: String) -> Self {
		Self(icon)
	}

	pub fn get_type(&self) -> IconType {
		if self.is_path() {
			IconType::Path
		} else if self.is_url() {
			IconType::Url
		} else {
			IconType::Stored
		}
	}

	pub fn is_path(&self) -> bool {
		self.0.starts_with("file://")
	}

	pub fn is_url(&self) -> bool {
		self.0.starts_with("http://") || self.0.starts_with("https://")
	}

	pub fn is_stored(&self) -> bool {
		!self.is_path() && !self.is_url()
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconType {
	Path,
	Url,
	Stored,
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
		Self::new(s)
	}
}

impl FromStr for Icon {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self::new(s.to_string()))
	}
}

impl Display for Icon {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Icon(IconType::{}, \"{}\")", self.get_type(), self.0)
	}
}
