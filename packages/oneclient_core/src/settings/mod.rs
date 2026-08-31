mod launcher;

pub use launcher::{DEFAULT_GRID_COLUMNS, LauncherSettings, ViewLayout, ViewState};
pub use oneclient_cluster::{
	GameSettingsProfile, PackageUpdateMode, ProfileUpdate, SettingsOsExtra,
};
pub use oneclient_common::Resolution;

pub mod store;

#[must_use]
pub fn net_config(settings: &LauncherSettings) -> oneclient_net::NetConfig {
	oneclient_net::NetConfig::default().with_overrides(
		settings.curseforge_api_key.as_deref(),
		settings.modrinth_api_key.as_deref(),
		settings.custom_api_endpoint.as_deref(),
		settings.custom_meta_url_base.as_deref(),
	)
}

