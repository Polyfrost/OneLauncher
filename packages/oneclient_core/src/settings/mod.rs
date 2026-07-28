mod launcher;

pub use launcher::{LauncherSettings, ViewLayout, ViewState};
// Profiles moved to `oneclient_cluster`; re-exported for existing paths.
pub use oneclient_cluster::{GameSettingsProfile, ProfileUpdate, SettingsOsExtra};
pub use oneclient_common::Resolution;

pub mod store;

/// The endpoint/credential subset of the settings, in the shape the transport
/// wants.
///
/// This is the seam that replaced `api_config`: rather than the HTTP client
/// reaching back into global settings (and therefore into the state object that
/// owns the client), the composition layer pushes these down whenever settings
/// change. See [`crate::settings::store::save_settings`].
#[must_use]
pub fn net_config(settings: &LauncherSettings) -> oneclient_net::NetConfig {
	oneclient_net::NetConfig::default().with_overrides(
		settings.curseforge_api_key.as_deref(),
		settings.modrinth_api_key.as_deref(),
		settings.custom_api_endpoint.as_deref(),
		settings.custom_meta_url_base.as_deref(),
	)
}

