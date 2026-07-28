use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{
    ChangelogGroup, LauncherError, fetch_changelog, parse_changelog,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChangelogKeys {
    pub meta_url_base: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChangelogQuery;

impl QueryCapability for ChangelogQuery {
    type Ok = Vec<ChangelogGroup>;
    type Err = LauncherError;
    type Keys = ChangelogKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = crate::launcher::state()?;
        let markdown = fetch_changelog(&state.services.requester).await?;
        Ok(parse_changelog(&markdown))
    }
}

pub fn use_changelog() -> UseQuery<ChangelogQuery> {
    let meta_url_base = super::use_meta_url_key();

    use_query(Query::new(ChangelogKeys { meta_url_base }, ChangelogQuery))
}

pub fn changelog_groups(query: &UseQuery<ChangelogQuery>) -> Option<Vec<ChangelogGroup>> {
    super::state::settled_or_loading(query)
}

pub fn latest_changelog_version(query: &UseQuery<ChangelogQuery>) -> Option<String> {
    changelog_groups(query).and_then(|groups| groups.first().map(|group| group.version.clone()))
}

pub fn changelog_error(query: &UseQuery<ChangelogQuery>) -> Option<String> {
    super::state::query_error(query)
}

pub fn changelog_is_loading(query: &UseQuery<ChangelogQuery>) -> bool {
    super::state::query_is_loading(query)
}
