use std::collections::HashMap;
use std::sync::Arc;

use super::{CurseForgeProvider, ModrinthProvider, PackageProvider};
use crate::LauncherResult;
use crate::packages::domain::ProviderId;
use crate::packages::error::PackageError;
use crate::packages::file_identity::FileIdentity;
use crate::packages::store::artifact_absolute_path;
use crate::packages::types::{VersionDetail, VersionLookup};
use crate::state::LauncherServices;

#[derive(Clone)]
pub struct PackageProviderRegistry {
    providers: HashMap<ProviderId, Arc<dyn PackageProvider>>,
}

impl PackageProviderRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };
        registry.register(Arc::new(ModrinthProvider));
        registry.register(Arc::new(CurseForgeProvider));
        registry
    }

    pub fn register(&mut self, provider: Arc<dyn PackageProvider>) {
        self.providers.insert(provider.id(), provider);
    }

    pub fn get(&self, id: ProviderId) -> LauncherResult<&dyn PackageProvider> {
        self.providers
            .get(&id)
            .map(|p| p.as_ref())
            .ok_or(PackageError::ProviderNotRegistered(id).into())
    }

    pub fn remote_ids(&self) -> Vec<ProviderId> {
        ProviderId::remote_providers()
            .iter()
            .copied()
            .filter(|id| self.providers.contains_key(id))
            .collect()
    }

    pub fn remote(&self) -> Vec<&dyn PackageProvider> {
        self.remote_ids()
            .into_iter()
            .filter_map(|id| self.providers.get(&id).map(|p| p.as_ref()))
            .collect()
    }

    pub async fn lookup_versions(
        &self,
        identities: &[FileIdentity],
        services: &LauncherServices,
    ) -> LauncherResult<VersionLookup> {
        if identities.is_empty() {
            return Ok(HashMap::new());
        }

        let mut enriched: Vec<FileIdentity> = identities.to_vec();
        for identity in &mut enriched {
            enrich_curseforge_fingerprint(identity, services).await?;
        }

        let mut merged = HashMap::new();
        for id in self.remote_ids() {
            let provider = self.get(id)?;
            let found = provider.lookup_versions(&enriched, services).await?;
            for (sha1, version) in found {
                merged.entry(sha1).or_insert(version);
            }
        }
        Ok(merged)
    }

    pub async fn lookup_version(
        &self,
        sha1: impl AsRef<str>,
        services: &LauncherServices,
    ) -> LauncherResult<Option<(ProviderId, VersionDetail)>> {
        let mut identity = FileIdentity::from_sha1(sha1);
        enrich_curseforge_fingerprint(&mut identity, services).await?;

        for id in self.remote_ids() {
            let provider = self.get(id)?;
            let mut found = provider
                .lookup_versions(std::slice::from_ref(&identity), services)
                .await?;
            if let Some(version) = found.remove(&identity.sha1) {
                return Ok(Some((id, version)));
            }
        }
        Ok(None)
    }

    pub async fn lookup_version_identity(
        &self,
        identity: &FileIdentity,
        services: &LauncherServices,
    ) -> LauncherResult<Option<(ProviderId, VersionDetail)>> {
        let mut identity = identity.clone();
        enrich_curseforge_fingerprint(&mut identity, services).await?;

        for id in self.remote_ids() {
            let provider = self.get(id)?;
            let mut found = provider
                .lookup_versions(std::slice::from_ref(&identity), services)
                .await?;
            if let Some(version) = found.remove(&identity.sha1) {
                return Ok(Some((id, version)));
            }
        }
        Ok(None)
    }
}

impl Default for PackageProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

async fn enrich_curseforge_fingerprint(
    identity: &mut FileIdentity,
    services: &LauncherServices,
) -> LauncherResult<()> {
    if identity.cf_fingerprint.is_some() {
        return Ok(());
    }

    let Some(row) =
        oneclient_db::dao::artifact::get_artifact_by_hash(&services.db, &identity.sha1).await?
    else {
        return Ok(());
    };

    let path = artifact_absolute_path(&row.path)?;
    if !path.exists() {
        return Ok(());
    }

    let bytes = polyio::read(&path).await?;
    identity.cf_fingerprint = Some(crate::packages::file_identity::curseforge_fingerprint(
        &bytes,
    ));
    Ok(())
}
