use freya::query::{
    QueriesStorage, Query, QueryCapability, QueryStateData, UseQuery, use_query,
};
use oneclient_core::LauncherState;
use oneclient_core::java::{AvailableJava, JavaManager, JavaRuntime, JavaVendor};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListJavaRuntimesQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListJavaRuntimesKeys;

impl QueryCapability for ListJavaRuntimesQuery {
    type Ok = Vec<JavaRuntime>;
    type Err = String;
    type Keys = ListJavaRuntimesKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get().map_err(|e| e.to_string())?;
        JavaManager::list_runtimes(&state.services.db)
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn use_java_runtimes() -> UseQuery<ListJavaRuntimesQuery> {
    use_query(Query::new(ListJavaRuntimesKeys, ListJavaRuntimesQuery))
}

pub fn java_runtimes(query: &UseQuery<ListJavaRuntimesQuery>) -> Vec<JavaRuntime> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. } => list.clone(),
        QueryStateData::Loading { res: Some(Ok(list)) } => list.clone(),
        _ => Vec::new(),
    }
}

pub async fn invalidate_java_queries() {
    QueriesStorage::<ListJavaRuntimesQuery>::try_invalidate_all().await;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProviderVersionsQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProviderVersionsKeys {
    pub vendor: JavaVendor,
}

impl QueryCapability for ProviderVersionsQuery {
    type Ok = Vec<AvailableJava>;
    type Err = String;
    type Keys = ProviderVersionsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get().map_err(|e| e.to_string())?;
        JavaManager::available_versions(&state.services, &keys.vendor)
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn use_provider_versions(vendor: JavaVendor) -> UseQuery<ProviderVersionsQuery> {
    use_query(Query::new(
        ProviderVersionsKeys { vendor },
        ProviderVersionsQuery,
    ))
}

pub fn provider_versions(
    query: &UseQuery<ProviderVersionsQuery>,
) -> (Vec<AvailableJava>, bool) {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled { res: Ok(list), .. } => (list.clone(), false),
        QueryStateData::Settled { res: Err(_), .. } => (Vec::new(), false),
        QueryStateData::Loading { res: Some(Ok(list)) } => (list.clone(), true),
        _ => (Vec::new(), true),
    }
}
