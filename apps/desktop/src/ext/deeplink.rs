use serde::{Deserialize, Serialize};

// TODO: this should be a specta event
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkEvent {
	data: String,
}
