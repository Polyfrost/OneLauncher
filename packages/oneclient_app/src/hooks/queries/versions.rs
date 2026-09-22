use std::collections::HashMap;
use std::sync::Arc;

use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_common::domain::GameLoader;
use oneclient_core::{GameVersionKind, LauncherError, VersionMetadata};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameVersion {
    pub id: String,
    pub kind: GameVersionKind,
    pub released: String,
}

impl QueryCapability for GameVersionsQuery {
    type Ok = Arc<[GameVersion]>;
    type Err = LauncherError;
    type Keys = GameVersionsKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;

        tokio::spawn(async move {
            let raw = {
                let mut metadata = state.metadata.lock().await;
                oneclient_core::get_version_ids(&mut metadata, &state.services.mc()).await?
            };

            let prepared: Arc<[GameVersion]> = raw
                .into_iter()
                .map(|info| GameVersion {
                    id: info.id,
                    kind: info.kind,
                    released: info.released.format("%d %b %Y").to_string(),
                })
                .collect();

            Ok(prepared)
        })
        .await
        .map_err(|err| LauncherError::Minecraft(err.to_string()))?
    }
}

pub fn use_game_versions() -> UseQuery<GameVersionsQuery> {
    use_query(Query::new(GameVersionsKeys, GameVersionsQuery))
}

pub fn game_versions(query: &UseQuery<GameVersionsQuery>) -> Option<Arc<[GameVersion]>> {
    super::state::settled_or_loading(query)
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
        let wanted = keys.versions.clone();

        tokio::spawn(async move {
            let mc = state.services.mc();

            let resolved = {
                let mut metadata = state.metadata.lock().await;
                let mut resolved = Vec::with_capacity(wanted.len());
                for id in &wanted {
                    if let Ok((version, _, _)) =
                        oneclient_core::game::resolve_minecraft_version(&mut metadata, &mc, id)
                            .await
                    {
                        resolved.push((id.clone(), version));
                    }
                }
                resolved
            };

            let lookups = resolved.into_iter().map(|(id, version)| {
                let mc = mc.clone();
                async move {
                    let info = oneclient_core::game::download_version_info(
                        &mc, None, &version, None, false,
                    )
                    .await
                    .ok()?;
                    Some((id, info.java_version?.major_version))
                }
            });

            let majors = futures_util::future::join_all(lookups)
                .await
                .into_iter()
                .flatten()
                .collect();

            Ok(majors)
        })
        .await
        .map_err(|err| LauncherError::Minecraft(err.to_string()))?
    }
}

pub fn use_java_majors(versions: Vec<String>) -> UseQuery<JavaMajorsQuery> {
    use_query(Query::new(JavaMajorsKeys { versions }, JavaMajorsQuery))
}

pub fn java_majors(query: &UseQuery<JavaMajorsQuery>) -> HashMap<String, u32> {
    super::state::settled_or_loading(query).unwrap_or_default()
}
