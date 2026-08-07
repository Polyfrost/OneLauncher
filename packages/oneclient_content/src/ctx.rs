use oneclient_db::DbPool;
use oneclient_events::EventBus;
use oneclient_net::RequestClient;

use crate::packages::provider::PackageProviderRegistry;

/// `db` is taken directly not behind a port
/// these tables are shared relational state with cross-entity joins used from
/// four subsystems
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
