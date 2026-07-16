use crate::constants;
use crate::settings::LauncherSettings;
use crate::state::LauncherState;

fn setting<T>(f: impl FnOnce(&LauncherSettings) -> Option<T>) -> Option<T> {
    let state = LauncherState::get().ok()?;
    let guard = state.settings.read();

    f(&guard)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub fn curseforge_api_key() -> String {
    non_empty(setting(|s| s.curseforge_api_key.clone()))
        .unwrap_or_else(|| constants::CURSEFORGE_API_KEY.to_string())
}

pub fn modrinth_api_key() -> Option<String> {
    non_empty(setting(|s| s.modrinth_api_key.clone()))
}

pub fn metadata_api_url() -> String {
    non_empty(setting(|s| s.custom_api_endpoint.clone()))
        .unwrap_or_else(|| constants::METADATA_API_URL.to_string())
}

pub fn meta_url_base() -> String {
    non_empty(setting(|s| s.custom_meta_url_base.clone()))
        .unwrap_or_else(|| constants::META_URL_BASE.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub fn modrinth_headers() -> Vec<(String, String)> {
    match modrinth_api_key() {
        Some(token) => vec![("Authorization".to_string(), token)],
        None => Vec::new(),
    }
}
