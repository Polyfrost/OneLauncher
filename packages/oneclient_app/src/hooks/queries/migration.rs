use std::time::Duration;

use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::{LauncherError, MigrationDetection, detect_migrations};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MigrationQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MigrationKeys;

impl QueryCapability for MigrationQuery {
    type Ok = Vec<MigrationDetection>;
    type Err = LauncherError;
    type Keys = MigrationKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        detect_migrations().await
    }
}

pub fn use_migration() -> UseQuery<MigrationQuery> {
    use_query(
        Query::new(MigrationKeys, MigrationQuery)
            .stale_time(Duration::from_secs(60 * 60))
            .clean_time(Duration::from_secs(6 * 60 * 60)),
    )
}

pub fn migration_detections(query: &UseQuery<MigrationQuery>) -> Vec<MigrationDetection> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(found), .. } => found.clone(),
        QueryStateData::Loading { res: Some(Ok(found)) } => found.clone(),
        _ => Vec::new(),
    }
}

pub fn has_migration_data(query: &UseQuery<MigrationQuery>) -> bool {
    migration_detections(query)
        .iter()
        .any(|d| !d.instances.is_empty())
}
