use onelauncher_core::entity::setting_profiles::Model;
use onelauncher_core::api::setting_profiles;

use crate::api::error::SerializableResult;

#[specta::specta]
#[tauri::command]
pub async fn get_global_profile() -> SerializableResult<Model> {
    Ok(setting_profiles::get_global_profile().await)
}

#[specta::specta]
#[tauri::command]
pub async fn get_profile_or_default(
    name: Option<String>,
) -> SerializableResult<Model> {
    Ok(setting_profiles::dao::get_profile_or_default(name.as_ref()).await?)
}