use std::collections::HashMap;

use crate::{utils::http, PolyResult};

use super::minecraft::MinecraftManifest;

pub async fn retrieve_versions() -> PolyResult<HashMap<String, String>> {
    let map = HashMap::new();

    Ok(map)
}

pub async fn retrieve_version_manifest(url: String) -> PolyResult<MinecraftManifest> {
    let manifest = http::create_client()?.get(url).send().await?.json::<MinecraftManifest>().await?;
    Ok(manifest)
}