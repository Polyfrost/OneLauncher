use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::{LauncherError, MigrationDetection, detect_migration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MigrationQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MigrationKeys;

impl QueryCapability for MigrationQuery {
    type Ok = Option<MigrationDetection>;
    type Err = LauncherError;
    type Keys = MigrationKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        detect_migration().await
    }
}

pub fn use_migration() -> UseQuery<MigrationQuery> {
    use_query(Query::new(MigrationKeys, MigrationQuery))
}

pub fn migration_detection(query: &UseQuery<MigrationQuery>) -> Option<MigrationDetection> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(opt), .. } => opt.clone(),
        QueryStateData::Loading { res: Some(Ok(opt)) } => opt.clone(),
        _ => None,
    }
}

pub fn has_migration_data(query: &UseQuery<MigrationQuery>) -> bool {
    migration_detection(query).is_some_and(|d| !d.instances.is_empty())
}
