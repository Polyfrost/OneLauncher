use freya::query::{QueriesStorage, Query, QueryCapability, UseQuery, use_query};
use oneclient_java::{AvailableJava, JavaRuntime, JavaVendor};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListJavaRuntimesQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListJavaRuntimesKeys;

impl QueryCapability for ListJavaRuntimesQuery {
    type Ok = Vec<JavaRuntime>;
    type Err = String;
    type Keys = ListJavaRuntimesKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state().map_err(|e| e.to_string())?;
        state.java.list_runtimes()
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn use_java_runtimes() -> UseQuery<ListJavaRuntimesQuery> {
    use_query(Query::new(ListJavaRuntimesKeys, ListJavaRuntimesQuery))
}

pub fn java_runtimes(query: &UseQuery<ListJavaRuntimesQuery>) -> Vec<JavaRuntime> {
    super::state::settled_or_loading(query).unwrap_or_default()
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
        let state = crate::launcher::state().map_err(|e| e.to_string())?;
        state.java.available_versions(&keys.vendor)
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

pub fn provider_versions(query: &UseQuery<ProviderVersionsQuery>) -> (Vec<AvailableJava>, bool) {
    (
        super::state::settled_or_loading(query).unwrap_or_default(),
        super::state::query_is_busy(query),
    )
}
