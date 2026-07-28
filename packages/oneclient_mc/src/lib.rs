//! Everything that makes a Minecraft install: the client jar, its libraries and
//! natives, the asset index and objects, the launch arguments assembled from
//! them, and the Mojang player profile.
//!
//! This crate has no database. It takes a [`RequestClient`](oneclient_net::RequestClient)
//! and an [`EventBus`](oneclient_events::EventBus) and works against paths, so
//! the download planner is testable on its own.

use oneclient_events::EventBus;
use oneclient_net::RequestClient;

/// What the install engine needs: somewhere to fetch from, and somewhere to
/// report progress to.
///
/// Narrower than the full launcher services: no database handle and no
/// package-provider registry, since none of this code touches them.
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
