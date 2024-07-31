use onelauncher::{data::MinecraftCredentials, minecraft};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[specta::specta]
#[tauri::command]
pub async fn get_users() -> Result<Vec<MinecraftCredentials>, String> {
	Ok(minecraft::users().await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_user(uuid: Uuid) -> Result<MinecraftCredentials, String> {
	Ok(minecraft::get_user(uuid).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn remove_user(uuid: Uuid) -> Result<(), String> {
	Ok(minecraft::remove_user(uuid).await?)
}

#[specta::specta]
#[tauri::command]
pub async fn get_default_user() -> Result<Option<MinecraftCredentials>, String> {
	let uuid = minecraft::get_default_user().await?;

	match uuid {
		Some(uuid) => Ok(Some(minecraft::get_user(uuid).await?)),
		None => Ok(None),
	}
}

#[specta::specta]
#[tauri::command]
pub async fn set_default_user(uuid: Uuid) -> Result<(), String> {
	minecraft::set_default_user(uuid).await?;
	Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn auth_login(handle: AppHandle) -> Result<Option<MinecraftCredentials>, String> {
	let flow = minecraft::begin().await?;
	let now = chrono::Utc::now();

	if let Some(win) = handle.get_webview_window("login") {
		win.close().map_err(|err| err.to_string())?;
	}

	let win = tauri::WebviewWindowBuilder::new(
		&handle,
		"login",
		tauri::WebviewUrl::External(
			flow.redirect_uri
				.parse()
				.map_err(|_| anyhow::anyhow!("failed to parse auth redirect url"))
				.map_err(|err| err.to_string())?,
		),
	)
	.title("Log into OneLauncher")
	.always_on_top(true)
	.center()
	.build()
	.map_err(|err| err.to_string())?;

	win.request_user_attention(Some(tauri::UserAttentionType::Critical))
		.map_err(|err| err.to_string())?;

	while (chrono::Utc::now() - now) < chrono::Duration::minutes(10) {
		if win.title().is_err() {
			return Ok(None);
		}

		if win
			.url()
			.map_err(|err| err.to_string())?
			.as_str()
			.starts_with("https://login.live.com/oauth20_desktop.srf")
		{
			if let Some((_, code)) = win
				.url()
				.map_err(|err| err.to_string())?
				.query_pairs()
				.find(|x| x.0 == "code")
			{
				win.close().map_err(|err| err.to_string())?;
				let value = minecraft::finish(&code.clone(), flow).await?;

				return Ok(Some(value));
			}
		}

		tokio::time::sleep(std::time::Duration::from_millis(50)).await;
	}

	win.close().map_err(|err| err.to_string())?;

	Ok(None)
}