use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{ExternalDetection, LauncherError, detect_external_launchers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExternalLaunchersQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExternalLaunchersKeys;

impl QueryCapability for ExternalLaunchersQuery {
    type Ok = Vec<ExternalDetection>;
    type Err = LauncherError;
    type Keys = ExternalLaunchersKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        Ok(crate::launcher::off_ui(detect_external_launchers()).await)
    }
}

pub fn use_external_launchers() -> UseQuery<ExternalLaunchersQuery> {
    use_query(Query::new(ExternalLaunchersKeys, ExternalLaunchersQuery))
}

/// `None` while the first scan is still running
pub fn external_launchers(
    query: &UseQuery<ExternalLaunchersQuery>,
) -> Option<Vec<ExternalDetection>> {
    super::state::settled_or_loading(query)
}
