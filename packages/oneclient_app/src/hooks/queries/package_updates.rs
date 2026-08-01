use std::collections::HashSet;

use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{BrowserPackageUpdate, LauncherError};
use oneclient_db::models::ClusterId;

/// What the last launch-time check recorded for one cluster.
///
/// Reads the cache table only: the check itself runs on launch, so rendering a
/// package list must never be what talks to a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageUpdatesQuery {
    pub cluster_id: ClusterId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackageUpdatesKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for PackageUpdatesQuery {
    type Ok = Vec<BrowserPackageUpdate>;
    type Err = LauncherError;
    type Keys = PackageUpdatesKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        Ok(oneclient_core::cached_browser_package_updates(
            self.cluster_id,
            &state.services.content(),
        )
        .await?)
    }
}

pub fn use_package_updates(cluster_id: ClusterId) -> UseQuery<PackageUpdatesQuery> {
    use_query(Query::new(
        PackageUpdatesKeys { cluster_id },
        PackageUpdatesQuery { cluster_id },
    ))
}

pub fn package_updates(query: &UseQuery<PackageUpdatesQuery>) -> Vec<BrowserPackageUpdate> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

/// The artifact hashes with a newer version waiting, for marking rows stale.
pub fn stale_hashes(query: &UseQuery<PackageUpdatesQuery>) -> HashSet<String> {
    package_updates(query)
        .into_iter()
        .map(|update| update.hash)
        .collect()
}
