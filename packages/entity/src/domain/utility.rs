use std::ops::Deref;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[onelauncher_macro::specta]
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct DbVec<T>(pub Vec<T>);

impl<T> Deref for DbVec<T> {
	type Target = Vec<T>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> From<Vec<T>> for DbVec<T> {
	fn from(v: Vec<T>) -> Self {
		Self(v)
	}
}

impl<T> sea_orm::TryGetableFromJson for DbVec<T> where for<'de> T: Deserialize<'de> {}

impl<T> std::convert::From<DbVec<T>> for sea_orm::Value
where
	DbVec<T>: Serialize,
{
	fn from(source: DbVec<T>) -> Self {
		Self::Json(serde_json::to_value(&source).ok().map(std::boxed::Box::new))
	}
}

impl<T> sea_orm::sea_query::ValueType for DbVec<T>
where
	Self: DeserializeOwned,
{
	fn try_from(v: sea_orm::Value) -> Result<Self, sea_orm::sea_query::ValueTypeErr> {
		match v {
			sea_orm::Value::Json(Some(json)) => {
				Ok(serde_json::from_value(*json).map_err(|_| sea_orm::sea_query::ValueTypeErr)?)
			}
			_ => Err(sea_orm::sea_query::ValueTypeErr),
		}
	}

	fn type_name() -> String {
		stringify!(#ident).to_owned()
	}

	fn array_type() -> sea_orm::sea_query::ArrayType {
		sea_orm::sea_query::ArrayType::Json
	}

	fn column_type() -> sea_orm::sea_query::ColumnType {
		sea_orm::sea_query::ColumnType::Json
	}
}

impl<T> sea_orm::sea_query::Nullable for DbVec<T> {
	fn null() -> sea_orm::Value {
		sea_orm::Value::Json(None)
	}
}
