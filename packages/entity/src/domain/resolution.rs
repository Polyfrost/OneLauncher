use sea_orm::TryGetableFromJson;
use sea_orm::sea_query::{Nullable, ValueType};
use serde::{Deserialize, Serialize};

#[onelauncher_macro::specta]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
	pub width: u32,
	pub height: u32,
}

impl Default for Resolution {
	fn default() -> Self {
		Self::new(854, 480)
	}
}

impl Resolution {
	#[must_use]
	pub const fn new(width: u32, height: u32) -> Self {
		Self { width, height }
	}
}

impl From<Resolution> for sea_orm::Value {
	fn from(value: Resolution) -> Self {
		Self::Json(serde_json::to_value(value).ok().map(Box::new))
	}
}

impl TryGetableFromJson for Resolution {}

impl ValueType for Resolution {
	fn try_from(v: sea_orm::Value) -> Result<Self, sea_orm::sea_query::ValueTypeErr> {
		match v {
			sea_orm::Value::Json(Some(json)) => {
				Ok(serde_json::from_value(*json).map_err(|_| sea_orm::sea_query::ValueTypeErr)?)
			}
			_ => Err(sea_orm::sea_query::ValueTypeErr),
		}
	}

	fn type_name() -> String {
		"Resolution".to_string()
	}

	fn array_type() -> sea_orm::sea_query::ArrayType {
		sea_orm::sea_query::ArrayType::Json
	}

	fn column_type() -> sea_orm::ColumnType {
		sea_orm::ColumnType::Json
	}
}

impl Nullable for Resolution {
	fn null() -> sea_orm::Value {
		sea_orm::Value::Json(None)
	}
}
