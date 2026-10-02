use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{DisableWarnings, LauncherError, fetch_disable_warnings};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DisableWarningsKeys {
    pub meta_url_base: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisableWarningsQuery;

impl QueryCapability for DisableWarningsQuery {
    type Ok = DisableWarnings;
    type Err = LauncherError;
    type Keys = DisableWarningsKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        fetch_disable_warnings(&state.services.requester).await
    }
}

pub fn use_disable_warnings() -> UseQuery<DisableWarningsQuery> {
    let meta_url_base = super::use_meta_url_key();

    use_query(Query::new(
        DisableWarningsKeys { meta_url_base },
        DisableWarningsQuery,
    ))
}

pub fn disable_warnings(query: &UseQuery<DisableWarningsQuery>) -> Option<DisableWarnings> {
    super::state::settled_or_loading(query)
}
