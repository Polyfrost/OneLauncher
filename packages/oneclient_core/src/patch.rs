use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Patch<T> {
	#[default]
	Unchanged,
	Clear,
	Set(T),
}

impl<T> Patch<T> {
	pub fn is_unchanged(&self) -> bool {
		matches!(self, Self::Unchanged)
	}

	pub fn apply_to_option(&self, slot: &mut Option<T>)
	where
		T: Clone,
	{
		match self {
			Self::Unchanged => {}
			Self::Clear => *slot = None,
			Self::Set(value) => *slot = Some(value.clone()),
		}
	}

	pub fn into_db_patch(self) -> Option<Option<T>> {
		match self {
			Self::Unchanged => None,
			Self::Clear => Some(None),
			Self::Set(value) => Some(Some(value)),
		}
	}
}

impl Patch<String> {
    pub fn apply_to_command_option(&self, slot: &mut Option<String>)
	{
		match self {
			Self::Unchanged => {}
			Self::Clear => *slot = None,
			Self::Set(value) => *slot = Some(value.trim().to_owned()),
		}
	}
}

impl Patch<&str> {
   pub fn apply_to_command_option(&self, slot: &mut Option<String>)
	{
		match self {
			Self::Unchanged => {}
			Self::Clear => *slot = None,
			Self::Set(value) => *slot = Some(value.trim().to_owned()),
		}
	}
}
