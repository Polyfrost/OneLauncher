use std::path::PathBuf;

use bytes::Bytes;
use freya::prelude::*;
use uuid::Uuid;

use crate::AppAssets;

pub fn use_player_skin(uuid: String) -> (Bytes, bool) {
    let profile = super::use_player_profile(uuid.clone(), None::<String>);

    let (skin_url, is_slim) = crate::hooks::settled_or_loading(&profile)
        .map_or((None, false), |profile| (profile.skin_url, profile.is_slim));

    // Offline accounts can carry custom textures the launcher applies itself
    // since their profiles have no session-server data
    let custom_skin = Uuid::parse_str(&uuid)
        .ok()
        .and_then(|id| super::try_account(&super::use_account(id)))
        .and_then(|account| account.custom_skin().cloned());

    let custom_is_slim = custom_skin.as_ref().and_then(|texture| {
        texture
            .model
            .as_deref()
            .map(|model| model == oneclient_auth::CustomTexture::SLIM)
    });

    let custom_url = custom_skin
        .as_ref()
        .map(|texture| texture.source.clone())
        .filter(|source| source.starts_with("http://") || source.starts_with("https://"));
    let custom_path = custom_skin
        .as_ref()
        .map(|texture| PathBuf::from(&texture.source))
        .filter(|path| path.is_absolute() && !path.as_os_str().is_empty());

    let custom_url_query = super::use_cached_image(custom_url.clone(), 256);
    let custom_path_query =
        super::use_local_image(custom_path.clone().unwrap_or_default(), 256, false, false);

    if custom_skin.is_some() {
        let bytes = if custom_url.is_some() {
            crate::hooks::loaded_image(custom_url.as_deref(), &custom_url_query)
                .map(|(_, bytes)| bytes)
        } else if custom_path.is_some() {
            crate::hooks::settled_or_loading(&custom_path_query)
        } else {
            None
        };

        if let Some(bytes) = bytes {
            let slim = custom_is_slim.unwrap_or(false);
            return (bytes, slim);
        }
    }

    let skin_query = super::use_cached_image(skin_url.clone(), 256);

    let steve = use_memo(|| AppAssets::get_bytes("steve.png").unwrap_or_default());
    let alex = use_memo(|| AppAssets::get_bytes("alex.png").unwrap_or_default());

    let default_slim = (java_string_hash(&uuid) & 1) == 1;

    match crate::hooks::loaded_image(skin_url.as_deref(), &skin_query) {
        Some((_, bytes)) => (bytes, is_slim),
        None if default_slim => (alex.read().clone(), true),
        None => (steve.read().clone(), false),
    }
}

fn java_string_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for c in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(c as i32);
    }
    h
}
