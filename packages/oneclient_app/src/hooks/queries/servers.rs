use freya::query::{QueriesStorage, Query, QueryCapability, UseQuery, use_query};
use oneclient_common::domain::ProviderId;
use oneclient_core::LauncherError;
use oneclient_core::servers::{self, BackupInfo, ListedPlayer, LocalServer, Properties, ServerMod};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListServersQuery;

impl QueryCapability for ListServersQuery {
    type Ok = Vec<LocalServer>;
    type Err = LauncherError;
    type Keys = ();

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        crate::launcher::off_ui(servers::list_servers()).await
    }
}

pub fn use_servers() -> UseQuery<ListServersQuery> {
    use_query(Query::new((), ListServersQuery))
}

pub fn use_server(id: &str) -> Option<LocalServer> {
    super::state::settled_or_loading(&use_servers())
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.id == id)
}

/// The icon's bytes keyed by their hash, so a changed icon is a new image to the renderer
/// instead of the cached picture of the same path
#[derive(Clone, Debug, PartialEq)]
pub struct ServerIcon {
    pub key: String,
    pub bytes: bytes::Bytes,
}

/// The small files the server pages read from the server folder
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ServerFiles {
    pub props: Properties,
    pub whitelist: Vec<ListedPlayer>,
    pub ops: Vec<ListedPlayer>,
    pub backups: Vec<BackupInfo>,
    pub world: String,
    pub icon: Option<ServerIcon>,
    pub eula: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServerFilesQuery;

impl QueryCapability for ServerFilesQuery {
    type Ok = ServerFiles;
    type Err = LauncherError;
    type Keys = String;

    async fn run(&self, id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let id = id.clone();
        crate::launcher::off_ui(async move {
            let dir = servers::server_dir(&id)?;
            let mc_version = servers::load_server(&id)
                .await
                .map(|s| s.mc_version)
                .unwrap_or_default();
            let icon = tokio::fs::read(dir.join(servers::ICON_FILE))
                .await
                .ok()
                .map(|bytes| ServerIcon {
                    key: format!("server-icon:{id}:{}", polyio::sha1_bytes(&bytes)),
                    bytes: bytes.into(),
                });
            let eula = tokio::fs::read_to_string(dir.join("eula.txt"))
                .await
                .is_ok_and(|text| text.lines().any(|l| l.trim() == "eula=true"));
            Ok(ServerFiles {
                props: Properties::read(&dir).await,
                whitelist: servers::read_list(&dir, servers::list_file(&mc_version, false)).await,
                ops: servers::read_list(&dir, servers::list_file(&mc_version, true)).await,
                backups: servers::list_backups(&dir).await,
                world: servers::world_name(&dir).await,
                icon,
                eula,
            })
        })
        .await
    }
}

pub fn use_server_files(id: String) -> UseQuery<ServerFilesQuery> {
    use_query(Query::new(id, ServerFilesQuery))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServerModsQuery;

impl QueryCapability for ServerModsQuery {
    type Ok = Vec<ServerMod>;
    type Err = LauncherError;
    type Keys = String;

    async fn run(&self, id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let id = id.clone();
        crate::launcher::off_ui(async move {
            let server = servers::load_server(&id).await?;
            Ok(servers::list_mods(&server.dir()?, server.content_folder()).await)
        })
        .await
    }
}

pub fn use_server_mods(id: String) -> UseQuery<ServerModsQuery> {
    use_query(Query::new(id, ServerModsQuery))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServerModUpdatesQuery;

impl QueryCapability for ServerModUpdatesQuery {
    type Ok = Vec<servers::ModUpdate>;
    type Err = LauncherError;
    type Keys = String;

    async fn run(&self, id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let id = id.clone();
        crate::launcher::off_ui(async move {
            let state = crate::launcher::state()?;
            let server = servers::load_server(&id).await?;
            servers::check_mod_updates(&state, &server).await
        })
        .await
    }
}

pub fn use_server_mod_updates(id: String) -> UseQuery<ServerModUpdatesQuery> {
    use_query(Query::new(id, ServerModUpdatesQuery))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PluginVersionsQuery;

impl QueryCapability for PluginVersionsQuery {
    type Ok = Vec<oneclient_content::packages::VersionSummary>;
    type Err = LauncherError;
    type Keys = Option<(ProviderId, String, String)>;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let Some((provider, project_id, mc_version)) = keys.clone() else {
            return Ok(Vec::new());
        };
        crate::launcher::off_ui(async move {
            let state = crate::launcher::state()?;
            servers::plugin_builds(&state, provider, &project_id, &mc_version).await
        })
        .await
    }
}

pub fn use_plugin_versions(
    keys: Option<(ProviderId, String, String)>,
) -> UseQuery<PluginVersionsQuery> {
    use_query(Query::new(keys, PluginVersionsQuery))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlatformAvailableQuery;

impl QueryCapability for PlatformAvailableQuery {
    type Ok = bool;
    type Err = LauncherError;
    type Keys = (servers::Platform, String);

    async fn run(&self, (platform, mc_version): &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let (platform, mc_version) = (*platform, mc_version.clone());
        crate::launcher::off_ui(async move {
            let state = crate::launcher::state()?;
            Ok(servers::platform_available(&state, platform, &mc_version).await)
        })
        .await
    }
}

pub fn use_platform_available(
    platform: servers::Platform,
    mc_version: String,
) -> UseQuery<PlatformAvailableQuery> {
    use_query(Query::new((platform, mc_version), PlatformAvailableQuery))
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ServerRun {
    pub world_size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServerRunQuery;

impl QueryCapability for ServerRunQuery {
    type Ok = ServerRun;
    type Err = LauncherError;
    type Keys = String;

    async fn run(&self, id: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let id = id.clone();
        crate::launcher::off_ui(async move {
            let dir = servers::server_dir(&id)?;
            let mut world_size = 0;
            for folder in servers::world_folders(&dir).await.unwrap_or_default() {
                world_size += servers::dir_size(dir.join(folder)).await;
            }
            Ok(ServerRun { world_size })
        })
        .await
    }
}

pub fn use_server_run(id: String) -> UseQuery<ServerRunQuery> {
    use_query(Query::new(id, ServerRunQuery))
}

pub async fn invalidate_server_settings(id: &str) {
    QueriesStorage::<ListServersQuery>::invalidate_all().await;
    QueriesStorage::<ServerFilesQuery>::invalidate_matching(id.to_string()).await;
}

pub async fn invalidate_server(id: &str) {
    invalidate_server_settings(id).await;
    QueriesStorage::<ServerModsQuery>::invalidate_matching(id.to_string()).await;
    QueriesStorage::<ServerModUpdatesQuery>::invalidate_matching(id.to_string()).await;
    QueriesStorage::<ServerRunQuery>::invalidate_matching(id.to_string()).await;
    QueriesStorage::<super::logs::ServerLogsQuery>::invalidate_matching(id.to_string()).await;
}
