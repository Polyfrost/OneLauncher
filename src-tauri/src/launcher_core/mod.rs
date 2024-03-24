use launcher_core::game::client::ClientManager;
use tauri::{plugin::TauriPlugin, Manager};
use tokio::sync::Mutex;

mod auth;
mod game;

pub struct GameManagerState {
    pub client_manager: ClientManager,
}

unsafe impl Send for GameManagerState {}
unsafe impl Sync for GameManagerState {}

pub fn init() -> TauriPlugin<tauri::Wry> {
	tauri::plugin::Builder::new("launcher-core")
		.setup(|app, _| {
			app.manage(Mutex::new(GameManagerState {
                client_manager: ClientManager::new().expect("Failed to initialize client manager"),
            }));
            
			Ok(())
		})
		.invoke_handler(tauri::generate_handler![
            auth::login_msa,
            game::create_instance,
            game::get_instances,
        ])
		.build()
}
