use std::path::PathBuf;

use freya::query::{
    Mutation, MutationCapability, QueriesStorage, Query, QueryCapability,
    UseMutation, UseQuery, use_mutation, use_query,
};
use oneclient_core::{
    LauncherError, LogFileInfo, LogLevel, LogLine, MclogsUploadResponse, ReadOptions,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterLogsKeys {
    pub cluster_id: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterLogsQuery;

impl QueryCapability for ClusterLogsQuery {
    type Ok = Vec<LogFileInfo>;
    type Err = LauncherError;
    type Keys = ClusterLogsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let cluster = state.clusters.get(keys.cluster_id).await?;
        Ok(oneclient_core::list_cluster_logs(&cluster)?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LogContentKeys {
    pub path: PathBuf,
    pub level: Option<LogLevel>,
    pub search: Option<String>,
    pub max_lines: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogContentQuery;

impl QueryCapability for LogContentQuery {
    type Ok = Vec<LogLine>;
    type Err = LauncherError;
    type Keys = LogContentKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        if keys.path.as_os_str().is_empty() {
            return Ok(Vec::new());
        }
        Ok(oneclient_core::read_log_at(
            &keys.path,
            &ReadOptions {
                level_filter: keys.level,
                search: keys.search.clone(),
                max_lines: keys.max_lines,
            },
        )
        .await?)
    }
}

pub fn use_cluster_logs(cluster_id: i64) -> UseQuery<ClusterLogsQuery> {
    use_query(Query::new(ClusterLogsKeys { cluster_id }, ClusterLogsQuery))
}

pub fn use_log_content(
    path: PathBuf,
    level: Option<LogLevel>,
    search: Option<String>,
    max_lines: Option<usize>,
) -> UseQuery<LogContentQuery> {
    use_query(Query::new(
        LogContentKeys {
            path,
            level,
            search,
            max_lines,
        },
        LogContentQuery,
    ))
}

pub fn try_cluster_logs(query: &UseQuery<ClusterLogsQuery>) -> Option<Vec<LogFileInfo>> {
    super::state::settled_or_loading(query)
}

pub fn try_log_content(query: &UseQuery<LogContentQuery>) -> Option<Vec<LogLine>> {
    super::state::settled_or_loading(query)
}

pub async fn invalidate_logs_queries() {
    QueriesStorage::<ClusterLogsQuery>::try_invalidate_all().await;
    QueriesStorage::<LogContentQuery>::try_invalidate_all().await;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UploadLogKeys {
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UploadLogMutation;

impl MutationCapability for UploadLogMutation {
    type Ok = MclogsUploadResponse;
    type Err = LauncherError;
    type Keys = UploadLogKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        Ok(oneclient_core::upload_log_at(&state.services.requester, &keys.path).await?)
    }
}

pub type UseUploadLog = UseMutation<UploadLogMutation>;

pub fn use_upload_log() -> UseUploadLog {
    use_mutation(Mutation::new(UploadLogMutation))
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LogAction {
    Delete { path: PathBuf },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogActionMutation;

impl MutationCapability for LogActionMutation {
    type Ok = ();
    type Err = LauncherError;
    type Keys = LogAction;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        match keys {
            LogAction::Delete { path } => Ok(oneclient_core::delete_log_at(path).await?),
        }
    }

    async fn on_settled(&self, _keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        if result.is_ok() {
            invalidate_logs_queries().await;
        }
    }
}

pub type UseLogAction = UseMutation<LogActionMutation>;

pub fn use_log_action() -> UseLogAction {
    use_mutation(Mutation::new(LogActionMutation))
}
