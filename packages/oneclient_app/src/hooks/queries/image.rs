use std::time::Duration;

use bytes::Bytes;
use freya::query::{Query, QueryCapability, UseQuery, use_query};
use oneclient_core::{LauncherError, LauncherState};

const IMAGE_STALE: Duration = Duration::from_secs(60 * 60);
const IMAGE_CLEAN: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CachedImageQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CachedImageKeys {
    pub url: String,
    pub max_edge: u32,
}

impl QueryCapability for CachedImageQuery {
    type Ok = Bytes;
    type Err = LauncherError;
    type Keys = CachedImageKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        if keys.url.is_empty() {
            return Err(LauncherError::Minecraft("no image url".to_string()));
        }

        let state = LauncherState::get()?;
        state
            .images
            .get(&state.services, &keys.url, keys.max_edge)
            .await
    }
}

pub fn use_cached_image(url: Option<String>, max_edge: u32) -> UseQuery<CachedImageQuery> {
    use_query(
        Query::new(
            CachedImageKeys {
                url: url.unwrap_or_default(),
                max_edge,
            },
            CachedImageQuery,
        )
        .stale_time(IMAGE_STALE)
        .clean_time(IMAGE_CLEAN),
    )
}
