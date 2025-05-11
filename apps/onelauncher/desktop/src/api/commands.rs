pub mod cluster;
pub mod users;
pub mod processor;
pub mod package;
pub mod other;

#[macro_export(local_inner_macros)]
macro_rules! collect_commands {
	() => {{
		use $crate::api::commands::*;
		tauri_specta::collect_commands![
			users::get_users,
			users::get_user,
			users::remove_user,
			users::get_default_user,
			users::set_default_user,
			users::begin_ms_flow,

			other::open_dev_tools,
		]
	}};
}
