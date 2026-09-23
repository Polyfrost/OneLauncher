use std::collections::HashMap;
use std::str::FromStr;

use interfrost::api::minecraft::VersionManifest as VanillaManifest;
use interfrost::api::modded::Manifest as ModdedManifest;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::McCtx;
use crate::error::{McError, McResult};
use oneclient_common::domain::GameLoader;
use oneclient_common::paths;

#[derive(Debug, Default)]
pub struct MetadataStore {
    initialized: bool,
    inner: MetadataInner,
    version_loader_cache: HashMap<String, Vec<GameLoader>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MetadataInner {
    minecraft: Option<VanillaManifest>,
    forge: Option<ModdedManifest>,
    neo: Option<ModdedManifest>,
    fabric: Option<ModdedManifest>,
    quilt: Option<ModdedManifest>,
    ornithe: Option<ModdedManifest>,
}

impl MetadataStore {
    #[must_use]
    pub const fn initialized(&self) -> bool {
        self.initialized
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    pub async fn get_vanilla_or_fetch(&mut self, ctx: &McCtx) -> McResult<&VanillaManifest> {
        if !self.initialized() {
            self.initialize(ctx).await?;
        }

        if self.inner.minecraft.is_none() {
            self.refetch_errored(ctx).await;
        }

        self.get_vanilla()
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    pub async fn get_modded_or_fetch(
        &mut self,
        ctx: &McCtx,
        loader: GameLoader,
    ) -> McResult<&ModdedManifest> {
        if !loader.is_modded() {
            return Err(McError::NotModdedManifest(loader));
        }

        if !self.initialized() {
            self.initialize(ctx).await?;
        }

        self.get_modded(loader)
    }

    pub fn get_vanilla(&self) -> McResult<&VanillaManifest> {
        self.inner
            .minecraft
            .as_ref()
            .ok_or_else(|| McError::FetchError)
    }

    pub fn get_modded(&self, loader: GameLoader) -> McResult<&ModdedManifest> {
        if !loader.is_modded() {
            return Err(McError::NotModdedManifest(loader));
        }

        match loader {
            GameLoader::Forge => self.inner.forge.as_ref(),
            GameLoader::NeoForge => self.inner.neo.as_ref(),
            GameLoader::Fabric => self.inner.fabric.as_ref(),
            GameLoader::Quilt => self.inner.quilt.as_ref(),
            GameLoader::Ornithe => self.inner.ornithe.as_ref(),
            GameLoader::Vanilla => None,
        }
        .ok_or_else(|| McError::FetchError)
    }

    #[tracing::instrument(skip_all)]
    pub async fn initialize(&mut self, ctx: &McCtx) -> McResult<()> {
        let path = paths::caches_dir()?.join("metadata.json");
        let mut save_file = false;
        let mut metadata = Self::default();

        match polyio::read_json::<MetadataInner>(&path).await {
            Ok(inner) => {
                metadata.inner = inner;

                if metadata.refetch_errored(ctx).await > 0 {
                    save_file = true;
                }
            }
            Err(err) => {
                if path.exists() {
                    tracing::warn!(
                        path = %path.display(),
                        "cached metadata manifest is unusable, refetching: {err}"
                    );
                }

                metadata.fetch_all(ctx).await;
                save_file = true;
            }
        }

        if save_file {
            polyio::write_json_atomic(&path, &metadata.inner).await?;
        }

        *self = metadata;
        self.initialized = true;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn refetch_errored(&mut self, ctx: &McCtx) -> u8 {
        let mut changed: u8 = 0;

        if self.inner.minecraft.is_none()
            && let Ok(data) = fetch_vanilla_manifest(ctx).await
        {
            self.inner.minecraft = Some(data);
            changed += 1;
        }

        macro_rules! check_modded {
            ($var:ident) => {
                if self.inner.$var.is_none() {
                    if let Ok(loader) = GameLoader::from_str(stringify!($var)) {
                        match fetch_modded_manifest(ctx, loader).await {
                            Ok(data) => {
                                self.inner.$var = Some(data);
                                changed += 1;
                            }
                            Err(err) => {
                                tracing::error!("failed to fetch manifest for {}: {}", loader, err);
                            }
                        }
                    }
                }
            };
        }

        check_modded!(forge);
        check_modded!(neo);
        check_modded!(fabric);
        check_modded!(quilt);
        check_modded!(ornithe);

        changed
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn fetch_all(&mut self, ctx: &McCtx) {
        let (minecraft, forge, neo, fabric, quilt, ornithe) = tokio::join!(
            fetch_vanilla_manifest(ctx),
            fetch_modded_manifest(ctx, GameLoader::Forge),
            fetch_modded_manifest(ctx, GameLoader::NeoForge),
            fetch_modded_manifest(ctx, GameLoader::Fabric),
            fetch_modded_manifest(ctx, GameLoader::Quilt),
            fetch_modded_manifest(ctx, GameLoader::Ornithe),
        );

        keep_fetched(&mut self.inner.minecraft, minecraft);
        keep_fetched(&mut self.inner.forge, forge);
        keep_fetched(&mut self.inner.neo, neo);
        keep_fetched(&mut self.inner.fabric, fabric);
        keep_fetched(&mut self.inner.quilt, quilt);
        keep_fetched(&mut self.inner.ornithe, ornithe);
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    pub async fn get_loaders_for_version(
        &mut self,
        ctx: &McCtx,
        mc_version: &str,
    ) -> McResult<Vec<GameLoader>> {
        if !self.initialized() {
            self.initialize(ctx).await?;
        }

        if let Some(hit) = self.version_loader_cache.get(mc_version) {
            return Ok(hit.clone());
        }

        let mut loaders = Vec::new();
        for loader in GameLoader::modded_loaders() {
            let Ok(manifest) = self.get_modded(*loader) else {
                continue;
            };

            if manifest_supports_version(manifest, mc_version) {
                loaders.push(*loader);
            }
        }

        self.version_loader_cache
            .insert(mc_version.to_owned(), loaders.clone());

        Ok(loaders)
    }

    #[tracing::instrument(level = "debug", skip(self, ctx))]
    pub async fn get_versions_for_loader(
        &mut self,
        ctx: &McCtx,
        loader: GameLoader,
    ) -> McResult<Option<Vec<String>>> {
        if !self.initialized() {
            self.initialize(ctx).await?;
        }

        if loader == GameLoader::Vanilla {
            return Ok(None);
        }

        let Ok(manifest) = self.get_modded(loader) else {
            return Ok(None);
        };

        Ok(concrete_version_ids(manifest))
    }
}

const LEGACY_DUMMY_REPLACE_STRING: &str = "${interpulse.gameVersion}";

#[must_use]
pub(crate) fn is_version_placeholder(entry_id: &str) -> bool {
    entry_id.contains(LEGACY_DUMMY_REPLACE_STRING)
        || entry_id.contains(interfrost::api::modded::DUMMY_REPLACE_STRING)
}

#[must_use]
pub(crate) fn entry_matches_version(entry_id: &str, mc_version: &str) -> bool {
    entry_id
        .replace(LEGACY_DUMMY_REPLACE_STRING, mc_version)
        .replace(interfrost::api::modded::DUMMY_REPLACE_STRING, mc_version)
        == mc_version
}

#[must_use]
pub(crate) fn concrete_version_ids(manifest: &ModdedManifest) -> Option<Vec<String>> {
    let mut ids = Vec::with_capacity(manifest.game_versions.len());

    for entry in &manifest.game_versions {
        if is_version_placeholder(&entry.id) {
            continue;
        }
        ids.push(entry.id.clone());
    }

    (!ids.is_empty()).then_some(ids)
}

#[must_use]
pub(crate) fn manifest_supports_version(manifest: &ModdedManifest, mc_version: &str) -> bool {
    let mut saw_concrete = false;

    for entry in &manifest.game_versions {
        if is_version_placeholder(&entry.id) {
            continue;
        }

        saw_concrete = true;

        if entry.id == mc_version {
            return true;
        }
    }

    !saw_concrete
}

fn keep_fetched<T>(slot: &mut Option<T>, fetched: McResult<T>) {
    if let Ok(data) = fetched {
        *slot = Some(data);
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn fetch_vanilla_manifest(ctx: &McCtx) -> McResult<VanillaManifest> {
    match fetch_manifest::<VanillaManifest>(ctx, GameLoader::Vanilla).await {
        Ok(manifest) => Ok(manifest),
        Err(err) => {
            tracing::warn!(
                "failed to fetch vanilla manifest from metadata mirror: {err}; falling back to Mojang"
            );

            {
                let url = interfrost::api::minecraft::VERSION_MANIFEST_URL
                    .parse()
                    .map_err(McError::Url)?;
                ctx.net
                    .send_json(Method::GET, url, None, &[])
                    .await
                    .map_err(McError::from)
            }
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn fetch_modded_manifest(ctx: &McCtx, loader: GameLoader) -> McResult<ModdedManifest> {
    fetch_manifest(ctx, loader).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn fetch_manifest<T: DeserializeOwned>(ctx: &McCtx, loader: GameLoader) -> McResult<T> {
    let url = format!(
        "{}/{}/v{}/manifest.json",
        ctx.net.config().metadata_api_url,
        loader.get_format_name(),
        loader.get_format_version()
    );

    let parsed = url.parse().map_err(McError::Url)?;
    ctx.net
        .send_json(Method::GET, parsed, None, &[])
        .await
        .map_err(McError::from)
}

#[cfg(test)]
mod tests {
    use super::{ModdedManifest, concrete_version_ids, manifest_supports_version};

    fn manifest(raw: &str) -> ModdedManifest {
        serde_json::from_str(raw).unwrap()
    }

    fn fabric_shaped() -> ModdedManifest {
        manifest(
            r#"{"gameVersions": [
                { "id": "${interfrost.gameVersion}", "stable": true, "loaders": [
                    { "id": "0.19.5", "url": "https://meta.example/0.19.5.json", "stable": true }
                ]},
                { "id": "1.21.1", "stable": true, "loaders": [] },
                { "id": "1.14", "stable": true, "loaders": [] }
            ]}"#,
        )
    }

    #[test]
    fn the_concrete_entries_are_the_supported_set() {
        let manifest = fabric_shaped();

        assert!(manifest_supports_version(&manifest, "1.21.1"));
        assert!(manifest_supports_version(&manifest, "1.14"));
        assert!(!manifest_supports_version(&manifest, "1.8.9"));
    }

    #[test]
    fn a_manifest_of_nothing_but_placeholders_covers_everything() {
        let manifest = manifest(
            r#"{"gameVersions": [
                { "id": "${interfrost.gameVersion}", "stable": true, "loaders": [] }
            ]}"#,
        );

        assert!(manifest_supports_version(&manifest, "1.8.9"));
        assert_eq!(concrete_version_ids(&manifest), None);
    }

    #[test]
    fn the_version_list_drops_the_placeholder_rather_than_going_unbounded() {
        assert_eq!(
            concrete_version_ids(&fabric_shaped()),
            Some(vec!["1.21.1".to_string(), "1.14".to_string()])
        );
    }
}
