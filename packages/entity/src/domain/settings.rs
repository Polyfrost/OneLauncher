use sea_orm::{sea_query::{self, Nullable, ValueType}, TryGetableFromJson};
use serde::{Deserialize, Serialize};

cfg_select! {
	target_os = "windows" => {
		#[onelauncher_macro::specta]
		#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
		pub struct SettingsOsExtra {

		}

		impl Default for SettingsOsExtra {
			fn default() -> Self {
				Self {

				}
			}
		}
	}
	target_os = "macos" => {
		#[onelauncher_macro::specta]
		#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
		pub struct SettingsOsExtra {

		}

		impl Default for SettingsOsExtra {
			fn default() -> Self {
				Self {

				}
			}
		}
	}
	not(any(target_os = "windows", target_os = "macos")) => {
		#[onelauncher_macro::specta]
		#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
		pub struct SettingsOsExtra {
			pub enable_gamemode: Option<bool>,
		}

		impl Default for SettingsOsExtra {
			fn default() -> Self {
				Self {
					enable_gamemode: Some(true),
				}
			}
		}
	}
}

impl From<SettingsOsExtra> for sea_query::Value {
	fn from(value: SettingsOsExtra) -> Self {
		sea_query::Value::Json(serde_json::to_value(value).ok().map(Box::new))
	}
}

impl TryGetableFromJson for SettingsOsExtra {}

impl ValueType for SettingsOsExtra {
	fn try_from(v: sea_orm::Value) -> Result<Self, sea_query::ValueTypeErr> {
		match v {
            sea_orm::Value::Json(Some(json)) => Ok(
                serde_json::from_value(*json).map_err(|_| sea_orm::sea_query::ValueTypeErr)?,
            ),
            _ => Err(sea_orm::sea_query::ValueTypeErr),
        }
	}

	fn type_name() -> String {
		"SettingsOsExtra".to_string()
	}

	fn array_type() -> sea_query::ArrayType {
		sea_query::ArrayType::Json
	}

	fn column_type() -> sea_orm::ColumnType {
		sea_orm::ColumnType::Json
	}
}

impl Nullable for SettingsOsExtra {
	fn null() -> sea_orm::Value {
		sea_orm::Value::Json(None)
	}
}