use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct JavaVersionRow {
	pub absolute_path: String,
	pub major: i64,
	pub version: String,
	pub vendor: String,
	pub os_arch: String,
}
