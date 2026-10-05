use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{ChangelogEntry, LauncherError, fetch_changelog};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChangelogQuery;

impl QueryCapability for ChangelogQuery {
    type Ok = Vec<ChangelogEntry>;
    type Err = LauncherError;
    type Keys = ();

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        fetch_changelog(&state.services.requester).await
    }
}

pub fn use_changelog() -> UseQuery<ChangelogQuery> {
    use_query(Query::new((), ChangelogQuery))
}

pub fn changelog_entries(query: &UseQuery<ChangelogQuery>) -> Option<Vec<ChangelogEntry>> {
    super::state::settled_or_loading(query)
}

pub fn latest_changelog_version(query: &UseQuery<ChangelogQuery>) -> Option<String> {
    changelog_entries(query).and_then(|entries| entries.first().map(|entry| entry.version.clone()))
}

pub fn changelog_error(query: &UseQuery<ChangelogQuery>) -> Option<String> {
    super::state::query_error(query)
}

pub fn changelog_is_loading(query: &UseQuery<ChangelogQuery>) -> bool {
    super::state::query_is_loading(query)
}
