mod launcher;

pub use launcher::{DEFAULT_GRID_COLUMNS, LauncherSettings, ViewLayout, ViewState};
pub use oneclient_cluster::{
	GameSettingsProfile, PackageUpdateMode, ProfileUpdate, SettingsOsExtra,
};
pub use oneclient_common::Resolution;

pub mod store;

/// Pushed down by the composition layer on every settings change so the HTTP
/// client never reaches back into global settings (and thus into its own owner)
#[must_use]
pub fn net_config(settings: &LauncherSettings) -> oneclient_net::NetConfig {
	oneclient_net::NetConfig::default().with_overrides(
		settings.curseforge_api_key.as_deref(),
		settings.modrinth_api_key.as_deref(),
		settings.custom_api_endpoint.as_deref(),
		settings.custom_meta_url_base.as_deref(),
	)
}

