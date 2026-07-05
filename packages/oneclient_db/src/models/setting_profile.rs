use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct SettingProfileRow {
	pub name: String,
	pub java_path: Option<String>,
	pub resolution: Option<String>,
	pub force_fullscreen: Option<i64>,
	pub mem_max: Option<i64>,
	pub launch_args: Option<String>,
	pub launch_env: Option<String>,
	pub hook_pre: Option<String>,
	pub hook_wrapper: Option<String>,
	pub hook_post: Option<String>,
	pub os_extra: Option<String>,
}
