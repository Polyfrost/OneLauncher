use std::time::Duration;

use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{
    LauncherError, LauncherState,
    minecraft::{self, PlayerProfileView},
};

// image cache
const PROFILE_STALE: Duration = Duration::from_secs(30 * 60);
const PROFILE_CLEAN: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FetchPlayerProfileQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlayerProfileQueryKeys {
    pub uuid: String,
    pub access_token: Option<String>,
}

impl QueryCapability for FetchPlayerProfileQuery {
    type Ok = PlayerProfileView;
    type Err = LauncherError;
    type Keys = PlayerProfileQueryKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let access_token = keys.access_token.clone();

        let state = LauncherState::get()?;
        let client = &state.services.requester;

        minecraft::fetch_player_profile_view(client, &keys.uuid, access_token.as_deref()).await
    }

    // TODO: Cache
    // fn matches(&self, keys: &Self::Keys) -> bool {
    //     keys.uuid.as_deref().is_some_and(|id| !id.is_empty())
    // }
}

pub fn use_player_profile(
    uuid: impl Into<String>,
    access_token: Option<impl Into<String>>,
) -> UseQuery<FetchPlayerProfileQuery> {
    use_query(
        Query::new(
            PlayerProfileQueryKeys {
                uuid: uuid.into(),
                access_token: access_token.map(|s| s.into()),
            },
            FetchPlayerProfileQuery,
        )
        .stale_time(PROFILE_STALE)
        .clean_time(PROFILE_CLEAN),
    )
}
