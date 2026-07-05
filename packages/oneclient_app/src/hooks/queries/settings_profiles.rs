use freya::query::{Query, QueryCapability, QueryStateData, UseQuery, use_query};
use oneclient_core::settings::GameSettingsProfile;
use oneclient_core::settings::store::{
    get_profile_or_default, list_named_profiles, resolve_cluster_profile,
};
use oneclient_core::{ClusterManager, LauncherState};
use oneclient_db::models::ClusterId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListNamedProfilesQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListNamedProfilesKeys;

impl QueryCapability for ListNamedProfilesQuery {
    type Ok = Vec<GameSettingsProfile>;
    type Err = String;
    type Keys = ListNamedProfilesKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let state = LauncherState::get().map_err(|e| e.to_string())?;
        list_named_profiles(&state.services.db)
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn use_named_profiles() -> UseQuery<ListNamedProfilesQuery> {
    use_query(Query::new(ListNamedProfilesKeys, ListNamedProfilesQuery))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GameProfileQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GameProfileKeys {
    pub name: Option<String>,
}

impl QueryCapability for GameProfileQuery {
    type Ok = GameSettingsProfile;
    type Err = String;
    type Keys = GameProfileKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let name = keys.name.clone();
        let state = LauncherState::get().map_err(|e| e.to_string())?;
        let settings = state.settings.read().clone();
        get_profile_or_default(&state.services.db, &settings, name.as_deref())
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn use_game_profile(name: Option<impl Into<String>>) -> UseQuery<GameProfileQuery> {
    use_query(Query::new(
        GameProfileKeys {
            name: name.map(Into::into),
        },
        GameProfileQuery,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterProfileQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClusterProfileKeys {
    pub profile_name: Option<String>,
}

impl QueryCapability for ClusterProfileQuery {
    type Ok = GameSettingsProfile;
    type Err = String;
    type Keys = ClusterProfileKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let profile_name = keys.profile_name.clone();
        let state = LauncherState::get().map_err(|e| e.to_string())?;
        let settings = state.settings.read().clone();
        resolve_cluster_profile(&state.services.db, &settings, profile_name.as_deref())
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn use_cluster_profile(
    profile_name: Option<impl Into<String>>,
) -> UseQuery<ClusterProfileQuery> {
    use_query(Query::new(
        ClusterProfileKeys {
            profile_name: profile_name.map(Into::into),
        },
        ClusterProfileQuery,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterSettingsQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClusterSettingsKeys {
    pub cluster_id: ClusterId,
}

impl QueryCapability for ClusterSettingsQuery {
    type Ok = GameSettingsProfile;
    type Err = String;
    type Keys = ClusterSettingsKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let cluster_id = keys.cluster_id;
        let state = LauncherState::get().map_err(|e| e.to_string())?;
        let cluster = ClusterManager::get(&state, cluster_id)
            .await
            .map_err(|e| e.to_string())?;
        ClusterManager::resolve_settings(&state, &cluster)
            .await
            .map_err(|e| e.to_string())
    }
}

pub fn use_cluster_settings(cluster_id: ClusterId) -> UseQuery<ClusterSettingsQuery> {
    use_query(Query::new(
        ClusterSettingsKeys { cluster_id },
        ClusterSettingsQuery,
    ))
}

pub fn try_game_profile(query: &UseQuery<GameProfileQuery>) -> Option<GameSettingsProfile> {
    let reader = query.read();
    match &*reader.state() {
        QueryStateData::Settled {
            res: Ok(profile), ..
        } => Some(profile.clone()),
        QueryStateData::Loading {
            res: Some(Ok(profile)),
        } => Some(profile.clone()),
        _ => None,
    }
}
