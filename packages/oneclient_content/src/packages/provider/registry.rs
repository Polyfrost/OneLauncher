use std::collections::HashMap;
use std::sync::Arc;


use super::{CurseForgeProvider, ModrinthProvider, PackageProvider};
use crate::error::ContentResult;
use oneclient_common::domain::ProviderId;
use crate::packages::error::PackageError;
use crate::packages::file_identity::FileIdentity;
use crate::packages::store::artifact_absolute_path;
use crate::packages::types::{ProviderVersionLookup, VersionDetail};
use crate::ctx::ContentCtx;

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

    pub fn get(&self, id: ProviderId) -> ContentResult<&dyn PackageProvider> {
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

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn lookup_versions(
        &self,
        identities: &[FileIdentity],
        ctx: &ContentCtx,
    ) -> ContentResult<ProviderVersionLookup> {
        if identities.is_empty() {
            return Ok(HashMap::new());
        }

        let mut enriched: Vec<FileIdentity> = identities.to_vec();
        let mut merged: ProviderVersionLookup = HashMap::new();

        for id in self.remote_ids() {
            if id == ProviderId::CurseForge {
                for identity in enriched
                    .iter_mut()
                    .filter(|identity| !merged.contains_key(&identity.sha1))
                {
                    if let Err(err) = enrich_curseforge_fingerprint(identity, ctx).await {
                        tracing::debug!(sha1 = %identity.sha1, "could not fingerprint: {err}");
                    }
                }
            }

            let provider = self.get(id)?;
            let found = match provider.lookup_versions(&enriched, ctx).await {
                Ok(found) => found,
                Err(err) => {
                    tracing::debug!(provider = ?id, "batch lookup failed: {err}");
                    continue;
                }
            };

            for (sha1, version) in found {
                merged.entry(sha1).or_insert((id, version));
            }
        }
        Ok(merged)
    }

    #[tracing::instrument(level = "debug", skip(self, sha1, ctx))]
    pub async fn lookup_version(
        &self,
        sha1: impl AsRef<str>,
        ctx: &ContentCtx,
    ) -> ContentResult<Option<(ProviderId, VersionDetail)>> {
        let identity = FileIdentity::from_sha1(sha1);
        self.first_match(identity, ctx).await
    }

    #[tracing::instrument(level = "debug", skip(self, ctx), fields(sha1 = %identity.sha1))]
    pub async fn lookup_version_identity(
        &self,
        identity: &FileIdentity,
        ctx: &ContentCtx,
    ) -> ContentResult<Option<(ProviderId, VersionDetail)>> {
        self.first_match(identity.clone(), ctx).await
    }

    async fn first_match(
        &self,
        mut identity: FileIdentity,
        ctx: &ContentCtx,
    ) -> ContentResult<Option<(ProviderId, VersionDetail)>> {
        for id in self.remote_ids() {
            if id == ProviderId::CurseForge {
                enrich_curseforge_fingerprint(&mut identity, ctx).await?;
            }

            let provider = self.get(id)?;
            let mut found = provider
                .lookup_versions(std::slice::from_ref(&identity), ctx)
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

#[tracing::instrument(level = "debug", skip(identity, ctx), fields(sha1 = %identity.sha1))]
async fn enrich_curseforge_fingerprint(
    identity: &mut FileIdentity,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    if identity.cf_fingerprint.is_some() {
        return Ok(());
    }

    let Some(row) =
        oneclient_db::dao::artifact::get_artifact_by_hash(&ctx.db, &identity.sha1).await?
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
