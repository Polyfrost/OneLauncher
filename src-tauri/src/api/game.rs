use std::path::PathBuf;

use onelauncher::{game::{clients::LaunchCallbacks, client_manager::ClusterWithManifest, clients::ClientType}, AppState};
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
pub async fn get_cluster_logs(
    state: State<'_, Mutex<AppState>>,
    uuid: Uuid,
) -> Result<Vec<PathBuf>, String> {
    let manager = &mut state.lock().await.clients;
    let cluster = manager.get_cluster(uuid)?;

    let log_files = cluster.cluster.get_log_files()?;
    Ok(log_files)
}

#[tauri::command]
pub async fn get_cluster_log(
    state: State<'_, Mutex<AppState>>,
    uuid: Uuid,
    log: PathBuf,
) -> Result<String, String> {
    let manager = &mut state.lock().await.clients;
    let cluster = manager.get_cluster(uuid)?;

    let log = cluster.cluster.get_log(&log)?;
    Ok(log)
}

#[tauri::command]
pub async fn launch_cluster(
    app: tauri::AppHandle,
	uuid: Uuid,
) -> Result<(), String> {
    let state = &mut app.state::<Mutex<AppState>>();
    let manager = &mut state.lock().await.clients;

    let on_launch_app = app.clone();
    let on_stdout_app = app.clone();
    let on_stderr_app = app.clone();
    let on_exit_app = app.clone();

    let callbacks = LaunchCallbacks {
        on_launch: Box::new(move |pid| {
            on_launch_app.emit("game:launch", pid).expect("Failed to emit game:launched");
        }),

        on_stdout: Box::new(move |pid, line| {
            on_stdout_app.emit("game:stdout", (pid, line)).expect("Failed to emit game:stdout");
        }),

        on_stderr: Box::new(move |pid, line| {
            on_stderr_app.emit("game:stderr", (pid, line)).expect("Failed to emit game:stderr");
        }),

        on_exit: Box::new(move |pid, exit_code| {
            on_exit_app.emit("game:exit", (pid, exit_code)).expect("Failed to emit game:exit");
        })
    };

    let _ = manager.launch(uuid, callbacks).await?;
    Ok(())
}
