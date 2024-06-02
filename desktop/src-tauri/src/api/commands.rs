use tauri_specta::{Builder, ExportLanguage};
use crate::api;

#[macro_export]
macro_rules! collect_commands {
    () => {
        {
            use crate::api::commands::*;
            tauri_specta::ts::builder()
                .commands(tauri_specta::collect_commands![
                    initialize_state,
                    is_dev
                ])
        }
    };
}

pub fn test() -> Builder {
    use crate::api::commands::*;
    tauri_specta::ts::builder()
        .commands(tauri_specta::collect_commands![
            initialize_state,
            is_dev
        ])
}

#[specta::specta]
#[tracing::instrument(skip_all)]
#[tauri::command]
pub async fn initialize_state(app: tauri::AppHandle) -> api::Result<()> {
	onelauncher::ProxyState::initialize(app).await?;
	let s = onelauncher::State::get().await?;
	onelauncher::State::update();

	s.processor.write().await.restore().await?;
	Ok(())
}

#[specta::specta]
#[tauri::command]
pub fn is_dev() -> bool {
	cfg!(debug_assertions)
}