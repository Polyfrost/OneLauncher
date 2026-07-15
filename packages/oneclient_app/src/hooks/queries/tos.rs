use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::{LauncherError, LauncherState, TermsDocument, fetch_terms};

use crate::hooks::use_settings_snapshot;

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
        let state = LauncherState::get()?;
        fetch_terms(&state.services).await
    }
}

pub fn use_terms() -> UseQuery<TermsQuery> {
    let settings = use_settings_snapshot().settings;
    let meta_url_base = settings
        .custom_meta_url_base
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();

    use_query(Query::new(TermsKeys { meta_url_base }, TermsQuery))
}

pub fn terms_document(query: &UseQuery<TermsQuery>) -> Option<TermsDocument> {
    match &*query.read().state() {
        QueryStateData::Settled {
            res: Ok(document), ..
        } => Some(document.clone()),
        QueryStateData::Loading {
            res: Some(Ok(document)),
        } => Some(document.clone()),
        _ => None,
    }
}

pub fn terms_error(query: &UseQuery<TermsQuery>) -> Option<String> {
    match &*query.read().state() {
        QueryStateData::Settled { res: Err(err), .. } => Some(err.to_string()),
        _ => None,
    }
}

pub fn terms_is_loading(query: &UseQuery<TermsQuery>) -> bool {
    matches!(
        &*query.read().state(),
        QueryStateData::Pending | QueryStateData::Loading { res: None }
    )
}
