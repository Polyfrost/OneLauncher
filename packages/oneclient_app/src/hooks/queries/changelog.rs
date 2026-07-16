use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::{
    ChangelogGroup, LauncherError, LauncherState, fetch_changelog, parse_changelog,
};

use crate::hooks::use_settings_snapshot;

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
        let state = LauncherState::get()?;
        let markdown = fetch_changelog(&state.services).await?;
        Ok(parse_changelog(&markdown))
    }
}

pub fn use_changelog() -> UseQuery<ChangelogQuery> {
    let settings = use_settings_snapshot().settings;
    let meta_url_base = settings
        .custom_meta_url_base
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();

    use_query(Query::new(ChangelogKeys { meta_url_base }, ChangelogQuery))
}

pub fn changelog_groups(query: &UseQuery<ChangelogQuery>) -> Option<Vec<ChangelogGroup>> {
    match &*query.read().state() {
        QueryStateData::Settled {
            res: Ok(groups), ..
        } => Some(groups.clone()),
        QueryStateData::Loading {
            res: Some(Ok(groups)),
        } => Some(groups.clone()),
        _ => None,
    }
}

pub fn latest_changelog_version(query: &UseQuery<ChangelogQuery>) -> Option<String> {
    changelog_groups(query).and_then(|groups| groups.first().map(|group| group.version.clone()))
}

pub fn changelog_error(query: &UseQuery<ChangelogQuery>) -> Option<String> {
    match &*query.read().state() {
        QueryStateData::Settled { res: Err(err), .. } => Some(err.to_string()),
        _ => None,
    }
}

pub fn changelog_is_loading(query: &UseQuery<ChangelogQuery>) -> bool {
    matches!(
        &*query.read().state(),
        QueryStateData::Pending | QueryStateData::Loading { res: None }
    )
}
