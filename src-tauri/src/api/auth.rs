use onelauncher::auth::{
	microsoft_auth::MicrosoftAuthenticationMethod, Account, AuthenticationMethod,
};
use tauri::Manager;

#[tauri::command]
pub async fn login_msa<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<Account, String> {
	match MicrosoftAuthenticationMethod::auth(|status, stage, was_last| {
		let _ = app.emit("msa:status", (status, stage, was_last));
	})
	.await
	{
		Ok(account) => Ok(account),
		Err(err) => Err(err.to_string()),
	}
}
