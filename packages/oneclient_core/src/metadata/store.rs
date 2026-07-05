use std::collections::HashMap;
use std::str::FromStr;

use interfrost::api::minecraft::VersionManifest as VanillaManifest;
use interfrost::api::modded::Manifest as ModdedManifest;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::api_config::metadata_api_url;
use crate::metadata::MetadataError;
use crate::packages::domain::GameLoader;
use crate::paths;
use crate::state::LauncherServices;
use crate::{LauncherError, LauncherResult};

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
    legacyfabric: Option<ModdedManifest>,
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

    #[instrument(skip(self, services))]
    pub async fn get_vanilla_or_fetch(
        &mut self,
        services: &LauncherServices,
    ) -> LauncherResult<&VanillaManifest> {
        if !self.initialized() {
            self.initialize(services).await?;
        }

        self.get_vanilla()
    }

    #[instrument(skip(self, services))]
    pub async fn get_modded_or_fetch(
        &mut self,
        services: &LauncherServices,
        loader: GameLoader,
    ) -> LauncherResult<&ModdedManifest> {
        if !loader.is_modded() {
            return Err(MetadataError::NotModdedManifest(loader).into());
        }

        if !self.initialized() {
            self.initialize(services).await?;
        }

        self.get_modded(loader)
    }

    pub fn get_vanilla(&self) -> LauncherResult<&VanillaManifest> {
        self.inner
            .minecraft
            .as_ref()
            .ok_or_else(|| MetadataError::FetchError.into())
    }

    pub fn get_modded(&self, loader: GameLoader) -> LauncherResult<&ModdedManifest> {
        if !loader.is_modded() {
            return Err(MetadataError::NotModdedManifest(loader).into());
        }

        match loader {
            GameLoader::Forge => self.inner.forge.as_ref(),
            GameLoader::NeoForge => self.inner.neo.as_ref(),
            GameLoader::Fabric => self.inner.fabric.as_ref(),
            GameLoader::Quilt => self.inner.quilt.as_ref(),
            GameLoader::LegacyFabric => self.inner.legacyfabric.as_ref(),
            GameLoader::Vanilla => None,
        }
        .ok_or_else(|| MetadataError::FetchError.into())
    }

    #[instrument(skip_all)]
    pub async fn initialize(&mut self, services: &LauncherServices) -> LauncherResult<()> {
        let path = paths::caches_dir()?.join("metadata.json");
        let mut save_file = false;
        let mut metadata = Self::default();

        if let Ok(bytes) = polyio::read(&path).await {
            if let Ok(inner) = serde_json::from_slice::<MetadataInner>(&bytes) {
                metadata.inner = inner;

                if metadata.refetch_errored(services).await > 0 {
                    save_file = true;
                }
            } else {
                metadata.fetch_all(services).await;
                save_file = true;
            }
        } else {
            metadata.fetch_all(services).await;
            save_file = true;
        }

        if save_file {
            if let Some(parent) = path.parent() {
                polyio::create_dir_all(parent).await?;
            }
            polyio::write(&path, &serde_json::to_vec(&metadata.inner)?).await?;
        }

        *self = metadata;
        self.initialized = true;

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn refetch_errored(&mut self, services: &LauncherServices) -> u8 {
        let mut changed: u8 = 0;

        if self.inner.minecraft.is_none()
            && let Ok(data) = fetch_vanilla_manifest(services).await {
                self.inner.minecraft = Some(data);
                changed += 1;
            }

        macro_rules! check_modded {
            ($var:ident) => {
                if self.inner.$var.is_none() {
                    if let Ok(loader) = GameLoader::from_str(stringify!($var)) {
                        match fetch_modded_manifest(services, loader).await {
                            Ok(data) => {
                                self.inner.$var = Some(data);
                                changed += 1;
                            }
                            Err(err) => {
                                tracing::error!(
                                    "failed to fetch manifest for {}: {}",
                                    loader,
                                    err
                                );
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
        check_modded!(legacyfabric);

        changed
    }

    #[instrument(skip_all)]
    pub async fn fetch_all(&mut self, services: &LauncherServices) {
        let (minecraft, forge, neo, fabric, quilt, legacyfabric) = tokio::join!(
            fetch_vanilla_manifest(services),
            fetch_modded_manifest(services, GameLoader::Forge),
            fetch_modded_manifest(services, GameLoader::NeoForge),
            fetch_modded_manifest(services, GameLoader::Fabric),
            fetch_modded_manifest(services, GameLoader::Quilt),
            fetch_modded_manifest(services, GameLoader::LegacyFabric),
        );

        self.inner.minecraft = minecraft.ok();
        self.inner.forge = forge.ok();
        self.inner.neo = neo.ok();
        self.inner.fabric = fabric.ok();
        self.inner.quilt = quilt.ok();
        self.inner.legacyfabric = legacyfabric.ok();
    }

    pub async fn get_loaders_for_version(
        &mut self,
        services: &LauncherServices,
        mc_version: &str,
    ) -> LauncherResult<Vec<GameLoader>> {
        if !self.initialized() {
            self.initialize(services).await?;
        }

        if let Some(hit) = self.version_loader_cache.get(mc_version) {
            return Ok(hit.clone());
        }

        let mut loaders = Vec::new();
        for loader in GameLoader::modded_loaders() {
            let manifest = match self.get_modded(*loader) {
                Ok(manifest) => manifest,
                Err(LauncherError::MetadataError(MetadataError::NotModdedManifest(_))) => {
                    continue
                }
                Err(e) => return Err(e),
            };

            let found = manifest.game_versions.iter().any(|entry| {
                entry
                    .id
                    .replace("${interpulse.gameVersion}", mc_version)
                    .replace(interfrost::api::modded::DUMMY_REPLACE_STRING, mc_version)
                    == mc_version
            });

            if found {
                loaders.push(*loader);
            }
        }

        self.version_loader_cache
            .insert(mc_version.to_owned(), loaders.clone());

        Ok(loaders)
    }
}

async fn fetch_vanilla_manifest(services: &LauncherServices) -> LauncherResult<VanillaManifest> {
    match fetch_manifest::<VanillaManifest>(services, GameLoader::Vanilla).await {
        Ok(manifest) => Ok(manifest),
        Err(err) => {
            tracing::warn!(
                "failed to fetch vanilla manifest from metadata mirror: {err}; falling back to Mojang"
            );

            {
                let url = interfrost::api::minecraft::VERSION_MANIFEST_URL
                    .parse()
                    .map_err(LauncherError::UrlError)?;
                services
                    .requester
                    .send_json(Method::GET, url, None, &[])
                    .await
                    .map_err(LauncherError::from)
            }
        }
    }
}

async fn fetch_modded_manifest(
    services: &LauncherServices,
    loader: GameLoader,
) -> LauncherResult<ModdedManifest> {
    fetch_manifest(services, loader).await
}

async fn fetch_manifest<T: DeserializeOwned>(
    services: &LauncherServices,
    loader: GameLoader,
) -> LauncherResult<T> {
    let url = format!(
        "{}/{}/v{}/manifest.json",
        metadata_api_url(),
        loader.get_format_name(),
        loader.get_format_version()
    );

    let parsed = url.parse().map_err(LauncherError::UrlError)?;
    services
        .requester
        .send_json(Method::GET, parsed, None, &[])
        .await
        .map_err(LauncherError::from)
}
