use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{LauncherError, TermsDocument, fetch_terms};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TermsKeys {
    pub meta_url_base: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TermsQuery;

impl QueryCapability for TermsQuery {
    type Ok = TermsDocument;
    type Err = LauncherError;
    type Keys = TermsKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        fetch_terms(&state.services.requester).await
    }
}

pub fn use_terms() -> UseQuery<TermsQuery> {
    let meta_url_base = super::use_meta_url_key();

    use_query(Query::new(TermsKeys { meta_url_base }, TermsQuery))
}

pub fn terms_document(query: &UseQuery<TermsQuery>) -> Option<TermsDocument> {
    super::state::settled_or_loading(query)
}

pub fn terms_error(query: &UseQuery<TermsQuery>) -> Option<String> {
    super::state::query_error(query)
}

pub fn terms_is_loading(query: &UseQuery<TermsQuery>) -> bool {
    super::state::query_is_loading(query)
}
