use onelauncher::game::client_manager::ClientManager;
use tauri::{plugin::TauriPlugin, Manager, State};
use tokio::sync::Mutex;

mod auth;
mod game;

pub struct GameManagerState {
	pub client_manager: ClientManager,
}

unsafe impl Send for GameManagerState {}
unsafe impl Sync for GameManagerState {}

pub fn init() -> TauriPlugin<tauri::Wry> {
	tauri::plugin::Builder::new("onelauncher")
		.setup(|app, _| {
			app.manage(Mutex::new(GameManagerState {
				client_manager: ClientManager::new().expect("Failed to initialize client manager"),
			}));

			Ok(())
		})
		.invoke_handler(tauri::generate_handler![
			auth::login_msa,
			game::create_cluster,
			game::get_clusters,
			game::get_cluster,
			game::get_manifest,
            game::launch_cluster,
			refresh_client_manager,
		])
		.build()
}

#[tauri::command]
#[tracing::instrument(skip_all)]
async fn refresh_client_manager(state: State<'_, Mutex<GameManagerState>>) -> Result<(), String> {
	let mut state = state.lock().await;
	state.client_manager = ClientManager::new()?;
	Ok(())
}
