use oneclient_db::DbPool;
use oneclient_events::EventBus;
use oneclient_net::RequestClient;

use crate::packages::provider::PackageProviderRegistry;

/// What the content subsystems need to do their work.
///
/// All four fields are used here, unlike the Java and Minecraft crates which
/// each need a strict subset.
///
/// The database handle is taken directly rather than behind a port. The tables
/// here (`artifacts`, `clusters`, `cluster_bundles`) are shared relational
/// state with cross-entity operations (`link_cluster_artifact`,
/// `is_cluster_linked`, `track_bundle_artifact`) used from four subsystems. A
/// trait over that would be the SQL schema with extra steps, and every schema
/// change would touch it. `java_versions` gets a port because it has one
/// owner and no joins; these do not.
#[derive(Clone)]
pub struct ContentCtx {
	pub db: DbPool,
	pub net: RequestClient,
	pub events: EventBus,
	pub providers: PackageProviderRegistry,
}

impl ContentCtx {
	#[must_use]
	pub fn new(
		db: DbPool,
		net: RequestClient,
		events: EventBus,
		providers: PackageProviderRegistry,
	) -> Self {
		Self {
			db,
			net,
			events,
			providers,
		}
	}
}
