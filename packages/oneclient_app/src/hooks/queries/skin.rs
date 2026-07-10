use bytes::Bytes;
use freya::prelude::*;
use freya::query::QueryStateData;

use crate::AppAssets;

pub fn use_player_skin(uuid: String) -> (Bytes, bool) {
    let profile = super::use_player_profile(uuid.clone(), None::<String>);

    let (skin_url, is_slim) = match &*profile.read().state() {
        QueryStateData::Settled {
            res: Ok(profile), ..
        } => (profile.skin_url.clone(), profile.is_slim),
        _ => (None, false),
    };

    let skin_query = super::use_cached_image(skin_url.clone(), 256);

    let steve = use_memo(|| AppAssets::get_bytes("steve.png").unwrap_or_default());
    let alex = use_memo(|| AppAssets::get_bytes("alex.png").unwrap_or_default());

    // No custom skin: pick alex (slim) or steve (classic) from the UUID.
    let default_slim = (java_string_hash(&uuid) & 1) == 1;

    let reader = skin_query.read();
    match (&skin_url, &*reader.state()) {
        (Some(_), QueryStateData::Settled { res: Ok(bytes), .. })
        | (
            Some(_),
            QueryStateData::Loading {
                res: Some(Ok(bytes)),
            },
        ) => (bytes.clone(), is_slim),
        _ if default_slim => (alex.read().clone(), true),
        _ => (steve.read().clone(), false),
    }
}

fn java_string_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for c in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(c as i32);
    }
    h
}