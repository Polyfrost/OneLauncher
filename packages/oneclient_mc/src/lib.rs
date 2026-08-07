//! No database here this crate works against paths so the download planner is
//! testable on its own

use oneclient_events::EventBus;
use oneclient_net::RequestClient;

#[derive(Clone)]
pub struct McCtx {
	pub net: RequestClient,
	pub events: EventBus,
}

impl McCtx {
	#[must_use]
	pub fn new(net: RequestClient, events: EventBus) -> Self {
		Self { net, events }
	}
}

mod arguments;
mod download;
mod error;
mod install;
mod manifest;
mod profile;
mod rules;

pub use arguments::*;
pub use download::{download_to_path, fetch_bytes_verified};
pub use error::{McError, McResult};
pub use install::*;
pub use manifest::MetadataStore;
pub use profile::{
	MojangCape, MojangFullPlayerProfile, MojangPlayerProfile, MojangSkin, PlayerProfileView,
	SkinVariant, fetch_logged_in_profile, fetch_player_profile, fetch_player_profile_view,
};
pub use rules::validate_rules;
