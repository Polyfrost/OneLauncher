use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::packages::domain::GameLoader;
use oneclient_core::{LauncherError, LauncherState, VersionMetadata};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VersionsMetadataQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VersionsMetadataKeys;

impl QueryCapability for VersionsMetadataQuery {
    type Ok = Vec<VersionMetadata>;
    type Err = LauncherError;
    type Keys = VersionsMetadataKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        let metadata = state.versions.metadata().await;
        if !metadata.is_empty() {
            return Ok(metadata);
        }
        state.versions.sync(&state.services).await?;
        Ok(state.versions.metadata().await)
    }
}

pub fn use_versions() -> UseQuery<VersionsMetadataQuery> {
    use_query(Query::new(VersionsMetadataKeys, VersionsMetadataQuery))
}

/// The manifest rows once the query has settled, `None` while it is still
/// loading. A failed fetch reads as an empty list rather than `None`, so
/// callers that block on this don't wait forever when the network is down.
pub fn versions_metadata(query: &UseQuery<VersionsMetadataQuery>) -> Option<Vec<VersionMetadata>> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. } => Some(list.clone()),
        QueryStateData::Settled { res: Err(_), .. } => Some(Vec::new()),
        QueryStateData::Loading {
            res: Some(Ok(list)),
        } => Some(list.clone()),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LoaderVersionsQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LoaderVersionsKeys {
    pub mc_version: String,
    pub loader: GameLoader,
}

impl QueryCapability for LoaderVersionsQuery {
    type Ok = Vec<String>;
    type Err = LauncherError;
    type Keys = LoaderVersionsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get()?;
        let mut metadata = state.metadata.lock().await;
        oneclient_core::get_loader_versions(
            &mut metadata,
            &state.services,
            &keys.mc_version,
            keys.loader,
        )
        .await
    }
}

pub fn use_loader_versions(
    mc_version: String,
    loader: GameLoader,
) -> UseQuery<LoaderVersionsQuery> {
    use_query(Query::new(
        LoaderVersionsKeys { mc_version, loader },
        LoaderVersionsQuery,
    ))
}

pub fn loader_versions(query: &UseQuery<LoaderVersionsQuery>) -> Vec<String> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. } => list.clone(),
        QueryStateData::Loading {
            res: Some(Ok(list)),
        } => list.clone(),
        _ => Vec::new(),
    }
}
