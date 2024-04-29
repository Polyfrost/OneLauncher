use onelauncher::game::client_manager::ClientManager;
use onelauncher::AppState;
use serde::{Serialize, Serializer};
use tauri::plugin::TauriPlugin;
use tauri::{Manager, State};
use tokio::sync::Mutex;

pub fn init() -> TauriPlugin<tauri::Wry> {
	tauri::plugin::Builder::new("onelauncher")
		.setup(|app, _| {
			app.manage(Mutex::new(AppState::new()?));

			Ok(())
		})
		.invoke_handler(tauri::generate_handler![])
		.build()
}

#[tauri::command]
#[tracing::instrument(skip_all)]
async fn refresh_client_manager(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
	let mut state = state.lock().await;
	state.clients = ClientManager::new()?;
	Ok(())
}

pub type Result<T> = std::result::Result<T, OneLauncherSerializableError>;

#[derive(thiserror::Error, Debug)]
pub enum OneLauncherSerializableError {
	#[error("{0}")]
	OneLauncher(#[from] onelauncher::Error),

	#[error("failed to handle io management: {0}")]
	IO(#[from] std::io::Error),

	#[error("failed to handle tauri management: {0}")]
	Tauri(#[from] tauri::Error),

	#[cfg(target_os = "macos")]
	#[error("failed to handle callback: {0}")]
	Callback(String),
}

// serialization code from https://github.com/modrinth/theseus/blob/master/theseus_gui/src-tauri/src/api/mod.rs
// (yoinked under MIT license)
macro_rules! impl_serialize_err {
	($($variant:ident),* $(,)?) => {
		impl Serialize for OneLauncherSerializableError {
			fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
			where
				S: Serializer,
			{
				match self {
					OneLauncherSerializableError::OneLauncher(onelauncher_error) => {
						$crate::error::display_tracing_error(onelauncher_error);

						let mut state = serializer.serialize_struct("OneLauncher", 2)?;
						state.serialize_field("field_name", "OneLauncher")?;
						state.serialize_field("message", &onelauncher_error.to_string())?;
						state.end()
					}
					$(
						OneLauncherSerializableError::$variant(message) => {
							let mut state = serializer.serialize_struct(stringify!($variant), 2)?;
							state.serialize_field("field_name", stringify!($variant))?;
							state.serialize_field("message", &message.to_string())?;
							state.end()
						},
					)*
				}
			}
		}
	};
}

#[cfg(target_os = "macos")]
impl_serialize_err! {
	IO,
	Tauri,
	Callback
}

#[cfg(not(target_os = "macos"))]
impl_serialize_err! {
	IO,
	Tauri,
}
