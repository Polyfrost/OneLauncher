use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::{
    LauncherError, LauncherState, MigrationDetection, detect_migration, resolve_migration_chain,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MigrationQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MigrationKeys;

impl QueryCapability for MigrationQuery {
    type Ok = Option<MigrationDetection>;
    type Err = LauncherError;
    type Keys = MigrationKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let Some(mut detection) = detect_migration().await? else {
            return Ok(None);
        };

        // remote cluster migrations run on startup (so before we even get to launcher migrations)
		// this pretty much tries to migrate the source instance to the latest version in the migration chain,
		// so that we can import it into the correct cluster
        let rules = LauncherState::get()?.versions.migrations().await;
        if !rules.is_empty() {
            for instance in &mut detection.instances {
                if instance.mc_version.is_empty() {
                    continue;
                }
                let resolved =
                    resolve_migration_chain(&instance.mc_version, instance.mc_loader, &rules);
                if resolved != instance.mc_version {
                    instance.target_mc_version = Some(resolved);
                }
            }
        }

        Ok(Some(detection))
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
