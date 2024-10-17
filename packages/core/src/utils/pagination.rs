use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Pagination {
	pub index: u32,
	pub page_size: u16,
	pub result_count: u32,
	pub total_count: u32,
}

pub fn get_page_offset(page: Option<u32>, page_size: Option<u16>) -> (u32, u16) {
	let page = page.unwrap_or(0);
	let page_size = page_size.unwrap_or(10);
	let offset = (page - 1) * page_size as u32;
	(offset, page_size)
}

pub fn get_page_count(total: u32, page_size: u16) -> u32 {
	(total as f64 / page_size as f64).ceil() as u32
}
