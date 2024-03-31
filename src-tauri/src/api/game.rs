use onelauncher::game::{client::{Cluster, LaunchCallbacks, Manifest}, clients::ClientType};
use tauri::{Manager, State};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::GameManagerState;

#[tauri::command]
pub async fn create_cluster(
	state: State<'_, Mutex<GameManagerState>>,
	name: String,
	version: String,
	client: ClientType,
	cover: Option<String>,
	group: Option<Uuid>,
) -> Result<Uuid, String> {
	let manager = &mut state.lock().await.client_manager;
	let uuid = manager
		.create_cluster(name, version, cover, group, client)
		.await?;
	Ok(uuid)
}

#[tauri::command]
pub async fn get_clusters(
	state: State<'_, Mutex<GameManagerState>>,
) -> Result<Vec<Cluster>, String> {
	let manager = &mut state.lock().await.client_manager;
	Ok(manager.get_clusters_owned())
}

#[tauri::command]
pub async fn get_cluster(
	state: State<'_, Mutex<GameManagerState>>,
	uuid: Uuid,
) -> Result<Cluster, String> {
	let manager = &mut state.lock().await.client_manager;
	let instance = manager.get_cluster(uuid)?;
	Ok(instance.clone())
}

#[tauri::command]
pub async fn launch_cluster(
    app: tauri::AppHandle,
    state: State<'_, Mutex<GameManagerState>>,
	uuid: Uuid,
) -> Result<i32, String> {
    let manager = &mut state.lock().await.client_manager;

    let on_launch_app = app.clone();
    let on_stdout_app = app.clone();
    let on_stderr_app = app.clone();

    let callbacks = LaunchCallbacks {
        on_launch: Box::new(move |pid| {
            on_launch_app.emit("game:launch", pid).expect("Failed to emit game:launched");
        }),

        on_stdout: Box::new(move |line| {
            on_stdout_app.emit("game:stdout", line).expect("Failed to emit game:stdout");
        }),

        on_stderr: Box::new(move |line| {
            on_stderr_app.emit("game:stderr", line).expect("Failed to emit game:stderr");
        })
    };

    let exit_code = manager.launch_cluster(uuid, callbacks).await?;
    Ok(exit_code)
}

#[tauri::command]
pub async fn get_manifest(
	state: State<'_, Mutex<GameManagerState>>,
	uuid: Uuid,
) -> Result<Manifest, String> {
	let manager = &mut state.lock().await.client_manager;
	let manifest = manager.get_manifest(uuid)?;
	Ok(manifest.clone())
}
