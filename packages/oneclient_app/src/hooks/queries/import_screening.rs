use std::path::PathBuf;

use freya::query::{QueriesStorage, Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{
    ExternalInstance, LauncherError, ScreenedInstance, screen_external_instances,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImportScreeningQuery {
    instances: Vec<ExternalInstance>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImportScreeningKeys {
    game_dirs: Vec<PathBuf>,
}

impl QueryCapability for ImportScreeningQuery {
    type Ok = Vec<ScreenedInstance>;
    type Err = LauncherError;
    type Keys = ImportScreeningKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        if self.instances.is_empty() {
            return Ok(Vec::new());
        }
        let state = crate::launcher::state()?;
        let instances = self.instances.clone();
        crate::launcher::off_ui(async move { screen_external_instances(&state, &instances).await })
            .await
    }
}

fn keys_for(instances: &[ExternalInstance]) -> ImportScreeningKeys {
    ImportScreeningKeys {
        game_dirs: instances.iter().map(|i| i.game_dir.clone()).collect(),
    }
}

pub fn use_import_screening(instances: Vec<ExternalInstance>) -> UseQuery<ImportScreeningQuery> {
    use_query(Query::new(
        keys_for(&instances),
        ImportScreeningQuery { instances },
    ))
}

pub fn import_screening(query: &UseQuery<ImportScreeningQuery>) -> Option<Vec<ScreenedInstance>> {
    super::state::settled_or_loading(query)
}

pub async fn retry_import_screening(instances: Vec<ExternalInstance>) {
    QueriesStorage::<ImportScreeningQuery>::invalidate_matching(keys_for(&instances)).await;
}
