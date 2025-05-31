use onelauncher_core::{api::credentials, error::LauncherResult, store::{credentials::{MinecraftCredentials, MinecraftLogin}, Core}};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use crate::api::error::SerializableResult;

#[specta::specta]
#[tauri::command]
pub async fn get_users() -> SerializableResult<Vec<MinecraftCredentials>> {
	Ok(credentials::users().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_user(uuid: Uuid) -> SerializableResult<MinecraftCredentials> {
	Ok(credentials::get_user(uuid).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn remove_user(uuid: Uuid) -> SerializableResult<()> {
	Ok(credentials::remove_user(uuid).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_default_user(
	fallback: Option<bool>,
) -> SerializableResult<Option<MinecraftCredentials>> {
	let uuid = credentials::get_default_user().await?;

	if fallback.is_some_and(|fallback| fallback) && uuid.is_none() {
		return Ok(credentials::users().await?.first().cloned());
	}

	match uuid {
		Some(uuid) => Ok(Some(credentials::get_user(uuid).await?)),
		None => Ok(None),
	}
}

#[specta::specta]
#[tauri::command]
pub async fn set_default_user(uuid: Option<Uuid>) -> SerializableResult<()> {
	credentials::set_default_user(uuid).await?;
	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn begin_ms_flow(handle: tauri::AppHandle) -> SerializableResult<Option<MinecraftCredentials>> {
	let flow = credentials::begin().await?;
	let result = spawn_webview(handle, flow).await?;

	Ok(result)
}

async fn spawn_webview(
	handle: AppHandle,
	flow: MinecraftLogin,
) -> LauncherResult<Option<MinecraftCredentials>> {
	let now = chrono::Utc::now();

	if let Some(win) = handle.get_webview_window("login") {
		win.close()?;
	}

	let win = tauri::WebviewWindowBuilder::new(
		&handle,
		"login",
		tauri::WebviewUrl::External(
			flow.redirect_uri
				.parse()
				.map_err(|_| anyhow::anyhow!("failed to parse auth redirect url"))?,
		),
	)
		.title(format!("Login to {}", Core::get().launcher_name))
		.center()
		.focused(true)
		.build()?;

	win.request_user_attention(Some(tauri::UserAttentionType::Critical))?;

	while (chrono::Utc::now() - now) < chrono::Duration::minutes(10) {
		if win.title().is_err() {
			return Ok(None);
		}

		if win
			.url()?
			.as_str()
			.starts_with("https://login.live.com/oauth20_desktop.srf")
		{
			if let Some((_, code)) = win
				.url()?
				.query_pairs()
				.find(|x| x.0 == "code")
			{
				win.close()?;
				let value = credentials::finish(&code.clone(), flow).await?;

				return Ok(Some(value));
			}
		}

		tokio::time::sleep(std::time::Duration::from_millis(50)).await;
	}

	win.close()?;

	Ok(None)
}
