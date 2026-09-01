use std::time::Duration;

use freya::query::{
    Mutation, MutationCapability, QueriesStorage, Query, QueryCapability, UseMutation, UseQuery,
    use_mutation, use_query,
};
use oneclient_core::LauncherError;
use oneclient_core::storage::{StorageReport, storage_report};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageReportKeys;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageReportQuery;

impl QueryCapability for StorageReportQuery {
    type Ok = StorageReport;
    type Err = LauncherError;
    type Keys = StorageReportKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        storage_report(&state).await
    }
}

const STORAGE_STALE_TIME: Duration = Duration::from_secs(120);

pub fn use_storage_report() -> UseQuery<StorageReportQuery> {
    use_query(Query::new(StorageReportKeys, StorageReportQuery).stale_time(STORAGE_STALE_TIME))
}

pub fn try_storage_report(query: &UseQuery<StorageReportQuery>) -> Option<StorageReport> {
    super::state::settled_or_loading(query)
}

pub async fn invalidate_storage_queries() {
    QueriesStorage::<StorageReportQuery>::invalidate_all().await;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StorageAction {
    CleanUnreferencedCache,
    CleanLegacyClusterContent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageActionMutation;

impl MutationCapability for StorageActionMutation {
    type Ok = ();
    type Err = String;
    type Keys = StorageAction;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state().map_err(|e| e.to_string())?;

        match keys {
            StorageAction::CleanUnreferencedCache => {
                oneclient_core::storage::clean_unreferenced_cache(&state)
                    .await
                    .map(|_| ())
            }
            StorageAction::CleanLegacyClusterContent => {
                oneclient_core::storage::clean_legacy_cluster_content(&state)
                    .await
                    .map(|_| ())
            }
        }
        .map_err(|e| e.to_string())
    }

    async fn on_settled(&self, _keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if result.is_ok() {
            invalidate_storage_queries().await;
        }
    }
}

pub type UseStorageAction = UseMutation<StorageActionMutation>;

pub fn use_storage_action() -> UseStorageAction {
    use_mutation(Mutation::new(StorageActionMutation))
}
