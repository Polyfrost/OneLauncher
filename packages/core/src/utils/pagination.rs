use std::fmt::Debug;
use std::marker::Send;
use serde::{Deserialize, Serialize};

#[onelauncher_macro::specta]
#[derive(Clone, Serialize, Deserialize)]
pub struct Paginated<T>
where T: Clone + Serialize + Send + Sync {
	pub total: usize,
	pub offset: usize,
	pub limit: usize,
	pub items: Vec<T>,
}

impl<T: Clone + Serialize + Send + Sync> Debug for Paginated<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Paginated")
			.field("total", &self.total)
			.field("offset", &self.offset)
			.field("limit", &self.limit)
			.field("items", &self.items.len())
			.finish()
	}
}

impl <T: Clone + Serialize + Send + Sync> Paginated<T> {
	#[must_use]
	pub const fn new(total: usize, offset: usize, limit: usize, items: Vec<T>) -> Self {
		Self { total, offset, limit, items }
	}
}
