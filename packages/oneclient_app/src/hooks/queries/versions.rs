use std::collections::HashMap;

use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_common::domain::GameLoader;
use oneclient_core::{GameVersionInfo, LauncherError, VersionMetadata};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VersionsMetadataQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VersionsMetadataKeys;

impl QueryCapability for VersionsMetadataQuery {
    type Ok = Vec<VersionMetadata>;
    type Err = LauncherError;
    type Keys = VersionsMetadataKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let metadata = state
            .versions
            .metadata(&state.services.requester.config().meta_url_base)
            .await;
        if !metadata.is_empty() {
            return Ok(metadata);
        }
        state.versions.sync(&state.services).await?;
        Ok(state
            .versions
            .metadata(&state.services.requester.config().meta_url_base)
            .await)
    }
}

pub fn use_versions() -> UseQuery<VersionsMetadataQuery> {
    use_query(Query::new(VersionsMetadataKeys, VersionsMetadataQuery))
}

/// A failed fetch reads as an empty list rather than `None` so callers that
/// block on this don't wait forever when the network is down
pub fn versions_metadata(query: &UseQuery<VersionsMetadataQuery>) -> Option<Vec<VersionMetadata>> {
    let reader = query.read();
    let state = reader.state();
    match &*state {
        QueryStateData::Settled { res: Err(_), .. } => Some(Vec::new()),
        other => other.ok().cloned(),
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
        let state = crate::launcher::state()?;
        let mut metadata = state.metadata.lock().await;
        Ok(oneclient_core::get_loader_versions(
            &mut metadata,
            &state.services.mc(),
            &keys.mc_version,
            keys.loader,
        )
        .await?)
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
    super::state::settled_or_loading(query).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameVersionsQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameVersionsKeys;

impl QueryCapability for GameVersionsQuery {
    type Ok = Vec<GameVersionInfo>;
    type Err = LauncherError;
    type Keys = GameVersionsKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let mut metadata = state.metadata.lock().await;
        Ok(oneclient_core::get_version_ids(&mut metadata, &state.services.mc()).await?)
    }
}

pub fn use_game_versions() -> UseQuery<GameVersionsQuery> {
    use_query(Query::new(GameVersionsKeys, GameVersionsQuery))
}

pub fn game_versions(query: &UseQuery<GameVersionsQuery>) -> Vec<GameVersionInfo> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VersionLoadersQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VersionLoadersKeys {
    pub mc_version: String,
}

impl QueryCapability for VersionLoadersQuery {
    type Ok = Vec<GameLoader>;
    type Err = LauncherError;
    type Keys = VersionLoadersKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let mut metadata = state.metadata.lock().await;
        Ok(oneclient_core::get_loaders_for_version(
            &mut metadata,
            &state.services.mc(),
            &keys.mc_version,
        )
        .await?)
    }
}

pub fn use_version_loaders(mc_version: String) -> UseQuery<VersionLoadersQuery> {
    use_query(Query::new(
        VersionLoadersKeys { mc_version },
        VersionLoadersQuery,
    ))
}

pub fn version_loaders(query: &UseQuery<VersionLoadersQuery>) -> Vec<GameLoader> {
    super::state::settled_or_loading(query).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LoaderGameVersionsQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LoaderGameVersionsKeys {
    pub loader: GameLoader,
}

impl QueryCapability for LoaderGameVersionsQuery {
    type Ok = Option<Vec<String>>;
    type Err = LauncherError;
    type Keys = LoaderGameVersionsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let mut metadata = state.metadata.lock().await;
        Ok(oneclient_core::get_versions_for_loader(
            &mut metadata,
            &state.services.mc(),
            keys.loader,
        )
        .await?)
    }
}

pub fn use_loader_game_versions(loader: GameLoader) -> UseQuery<LoaderGameVersionsQuery> {
    use_query(Query::new(
        LoaderGameVersionsKeys { loader },
        LoaderGameVersionsQuery,
    ))
}

pub fn loader_game_versions(
    query: &UseQuery<LoaderGameVersionsQuery>,
) -> Option<Option<Vec<String>>> {
    super::state::settled_or_loading(query)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JavaMajorsQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct JavaMajorsKeys {
    pub versions: Vec<String>,
}

impl QueryCapability for JavaMajorsQuery {
    type Ok = HashMap<String, u32>;
    type Err = LauncherError;
    type Keys = JavaMajorsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        if keys.versions.is_empty() {
            return Ok(HashMap::new());
        }

        let state = crate::launcher::state()?;
        let mc = state.services.mc();

        let resolved = {
            let mut metadata = state.metadata.lock().await;
            let mut resolved = Vec::with_capacity(keys.versions.len());
            for id in &keys.versions {
                if let Ok((version, _, _)) =
                    oneclient_core::game::resolve_minecraft_version(&mut metadata, &mc, id).await
                {
                    resolved.push((id.clone(), version));
                }
            }
            resolved
        };

        let mut majors = HashMap::with_capacity(resolved.len());
        for (id, version) in resolved {
            if let Ok(info) =
                oneclient_core::game::download_version_info(&mc, None, &version, None, false).await
                && let Some(java) = info.java_version
            {
                majors.insert(id, java.major_version);
            }
        }

        Ok(majors)
    }
}

pub fn use_java_majors(versions: Vec<String>) -> UseQuery<JavaMajorsQuery> {
    use_query(Query::new(JavaMajorsKeys { versions }, JavaMajorsQuery))
}

pub fn java_majors(query: &UseQuery<JavaMajorsQuery>) -> HashMap<String, u32> {
    super::state::settled_or_loading(query).unwrap_or_default()
}
