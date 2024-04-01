use onelauncher::{game::{client::LaunchCallbacks, client_manager::ClusterWithManifest, clients::ClientType}, AppState};
use tauri::{Manager, State};
use tokio::sync::Mutex;
use uuid::Uuid;

#[tauri::command]
pub async fn create_cluster(
	state: State<'_, Mutex<AppState>>,
	name: String,
	version: String,
	client: ClientType,
	cover: Option<String>,
	group: Option<Uuid>,
) -> Result<Uuid, String> {
	let manager = &mut state.lock().await.clients;
	let uuid = manager
		.create_cluster(name, version, cover, group, client)
		.await?;
	Ok(uuid)
}

#[tauri::command]
pub async fn get_clusters(
	state: State<'_, Mutex<AppState>>,
) -> Result<Vec<ClusterWithManifest>, String> {
	let manager = &mut state.lock().await.clients;
	Ok(manager.get_clusters_owned())
}

#[tauri::command]
pub async fn get_cluster(
	state: State<'_, Mutex<AppState>>,
	uuid: Uuid,
) -> Result<ClusterWithManifest, String> {
	let manager = &mut state.lock().await.clients;
	let cluster = manager.get_cluster(uuid)?;
	Ok(cluster.clone())
}

#[tauri::command]
pub async fn launch_cluster(
    app: tauri::AppHandle,
    state: State<'_, Mutex<AppState>>,
	uuid: Uuid,
) -> Result<i32, String> {
    let manager = &mut state.lock().await.clients;

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
