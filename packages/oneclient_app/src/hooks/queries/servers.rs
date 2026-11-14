
use freya::query::{QueriesStorage, Query, QueryCapability, UseQuery, use_query};
use oneclient_core::LauncherError;
use oneclient_core::servers::{self, BackupInfo, ListedPlayer, LocalServer, Properties};

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

/// Everything the server pages read from the server folder
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ServerFiles {
    pub props: Properties,
    pub whitelist: Vec<ListedPlayer>,
    pub ops: Vec<ListedPlayer>,
    pub backups: Vec<BackupInfo>,
    pub mods: Vec<servers::ServerMod>,
    pub world: String,
    pub world_size: u64,
    pub icon: Option<ServerIcon>,
    pub eula: bool,
    pub latest_log: Vec<String>,
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
            let world = servers::world_name(&dir).await;
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
            let latest_log = tokio::fs::read_to_string(dir.join("logs").join("latest.log"))
                .await
                .map(|text| text.lines().map(str::to_string).collect())
                .unwrap_or_default();
            Ok(ServerFiles {
                props: Properties::read(&dir).await,
                whitelist: servers::read_list(&dir, servers::WHITELIST).await,
                ops: servers::read_list(&dir, servers::OPS).await,
                backups: servers::list_backups(&dir).await,
                mods: servers::list_mods(&dir).await,
                world_size: servers::dir_size(dir.join(&world)).await,
                world,
                icon,
                eula,
                latest_log,
            })
        })
        .await
    }
}

pub fn use_server_files(id: String) -> UseQuery<ServerFilesQuery> {
    use_query(Query::new(id, ServerFilesQuery))
}

pub async fn invalidate_server_queries() {
    QueriesStorage::<ListServersQuery>::invalidate_all().await;
    QueriesStorage::<ServerFilesQuery>::invalidate_all().await;
}
