use onelauncher::{game::client_manager::ClientManager, AppState};
use tauri::{plugin::TauriPlugin, Manager, State};
use tokio::sync::Mutex;

mod auth;
mod game;

pub fn init() -> TauriPlugin<tauri::Wry> {
	tauri::plugin::Builder::new("onelauncher")
		.setup(|app, _| {
			app.manage(Mutex::new(AppState::new()?));

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
async fn refresh_client_manager(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
	let mut state = state.lock().await;
	state.clients = ClientManager::new()?;
	Ok(())
}
