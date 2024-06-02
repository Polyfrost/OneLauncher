use crate::api;

#[macro_export]
macro_rules! collect_commands {
    () => {
        {
            use crate::api::commands::*;
            tauri_specta::ts::builder()
                .commands(tauri_specta::collect_commands![
                    is_dev
                ])
        }
    };
}

#[specta::specta]
#[tauri::command]
pub fn is_dev() -> bool {
	cfg!(debug_assertions)
}