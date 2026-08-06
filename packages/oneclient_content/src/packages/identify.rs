//! Recognising a file on disk as a package a provider publishes.
//!
//! A jar the user drops into a cluster arrives with nothing but its bytes, and
//! the launcher treats it accordingly: `ProviderId::Local`, no project, no
//! version, no place in the update check and no way for
//! [`installed_copies`](super::installed_copies) to tell it apart from anything
//! else. But both providers will name that file if asked — Modrinth by SHA-1,
//! CurseForge by the Murmur2 fingerprint in
//! [`curseforge_fingerprint`](super::curseforge_fingerprint) — and a file they
//! name is a browsed package that merely came in through the wrong door.
//!
//! # This runs on install and nowhere else
//!
//! Every function here goes to the network, once per file. That is affordable
//! when the user has just asked for something and is watching it happen. It is
//! not affordable on the paths that would otherwise reach for it:
//!
//! * the folder scan in `oneclient_core::game::shared_dir` runs at every launch
//!   of every cluster, over every loose file, whether or not anything changed;
//! * startup recovery runs over an entire reconstructed library at once;
//! * a render pass runs whenever anything on screen moves.
//!
//! Two things keep it there. Nothing in this module can be reached without an
//! [`InstallIntent`], which is a nuisance to produce by accident and reads as a
//! claim at the call site; and identification is deliberately *not* folded into
//! [`PackageStore::import_local_file`](super::PackageStore::import_local_file),
//! which is what the scan and recovery call, so no amount of work on those paths
//! turns into provider traffic on its own. Grep for `InstallIntent` to see every
//! place the launcher spends a lookup.

use std::collections::HashMap;
use std::path::PathBuf;

use oneclient_common::domain::ProviderId;
use oneclient_db::dao::artifact as artifact_dao;

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::file_identity::FileIdentity;
use crate::packages::types::VersionDetail;

/// A claim, made at the call site, that the user asked for this.
///
/// Carries nothing. It exists so that "does this path hash files at the
/// provider?" is answerable by reading the signature rather than by following
/// the call graph, and so that a scan cannot pick up a lookup by refactoring its
/// way into a function that already had a `ContentCtx` to hand.
#[derive(Debug, Clone, Copy)]
pub struct InstallIntent(());

impl InstallIntent {
    /// Call only from a path the user started: an install, an import, a drop.
    ///
    /// Not from a scan, from startup recovery, or from anything that runs on a
    /// timer or a render.
    pub fn user_initiated() -> Self {
        Self(())
    }
}

/// A local file the providers turned out to know.
#[derive(Debug, Clone)]
pub struct IdentifiedPackage {
    pub provider: ProviderId,
    pub version: VersionDetail,
}

impl IdentifiedPackage {
    /// What to call this in a warning, matching the label
    /// [`InstalledCopy`](super::InstalledCopy) builds for a copy already in the
    /// cluster, so the two read as the same kind of thing.
    pub fn label(&self) -> String {
        format!("{} {}", self.version.name, self.version.version_number)
    }
}

/// Asks the providers what they know about each of `paths`.
///
/// The answer is positional: `result[i]` is about `paths[i]`, and `None` means
/// no provider claimed it — which is the ordinary case for a config, a resource
/// pack someone made, or anything that has been repackaged since it was
/// published. A file that cannot be read is `None` too; being unable to identify
/// something is never a reason to hold up the install of it.
///
/// One round trip per provider for the whole batch, not per file: dropping
/// twenty jars is one question asked of Modrinth and one of CurseForge.
#[tracing::instrument(level = "debug", skip(paths, _intent, ctx))]
pub async fn identify_for_install(
    paths: &[PathBuf],
    _intent: InstallIntent,
    ctx: &ContentCtx,
) -> ContentResult<Vec<Option<IdentifiedPackage>>> {
    // Both hashes come off one read here, so CurseForge is asked about a dropped
    // file on the same terms as Modrinth. The registry's own fallback — reading
    // the cached artifact back to fingerprint it — cannot help a file that has
    // not been imported yet.
    let mut identities = Vec::with_capacity(paths.len());
    for path in paths {
        match FileIdentity::from_path(path).await {
            Ok(identity) => identities.push(Some(identity)),
            Err(err) => {
                tracing::warn!(path = %path.display(), %err, "could not read file to identify it");
                identities.push(None);
            }
        }
    }

    let readable: Vec<FileIdentity> = identities.iter().flatten().cloned().collect();
    let found = ctx.providers.lookup_identified(&readable, ctx).await?;

    Ok(align(&identities, &found))
}

/// Puts the lookup's answers back in the order the caller asked in.
///
/// The unreadable files were dropped before the request went out, so the
/// response is shorter than the input and cannot be zipped against it. Getting
/// this wrong would not fail: it would quietly record one dropped jar as
/// another, which is the sort of mistake that only surfaces when the update
/// check offers the user a version of something they never installed.
fn align(
    identities: &[Option<FileIdentity>],
    found: &HashMap<String, (ProviderId, VersionDetail)>,
) -> Vec<Option<IdentifiedPackage>> {
    identities
        .iter()
        .map(|identity| {
            let identity = identity.as_ref()?;
            found
                .get(&identity.sha1)
                .map(|(provider, version)| IdentifiedPackage {
                    provider: *provider,
                    version: version.clone(),
                })
        })
        .collect()
}

/// Writes an identified artifact into the table that makes it a browsed package.
///
/// `provider_releases` is the whole of the difference. Everything downstream —
/// the provider and version tags,
/// [`list_linked_artifacts`](super::PackageStore::list_linked_artifacts), the
/// update check, [`installed_copies`](super::installed_copies) — reads the
/// artifact's release row by hash, so a local import with one is a browsed
/// package in every respect and one without is not, whatever else is recorded
/// about it.
///
/// Called after the import, because the row is keyed on the artifact's hash and
/// there is no artifact until then.
#[tracing::instrument(level = "debug", skip(identified, ctx), fields(provider = ?identified.provider))]
pub async fn record_identified_package(
    hash: &str,
    identified: &IdentifiedPackage,
    ctx: &ContentCtx,
) -> ContentResult<()> {
    let version = &identified.version;

    artifact_dao::upsert_provider_release(
        &ctx.db,
        identified.provider as i64,
        &version.project_id,
        &version.version_id,
        hash,
        &version.name,
        &version.version_number,
        Some(version.published.to_rfc3339().as_str()),
        &serde_json::to_string(&version.game_versions)?,
        &serde_json::to_string(&version.loaders)?,
    )
    .await?;

    Ok(())
}

/// Whether this artifact is already a browsed package.
///
/// A file the user re-imports over one the launcher downloaded arrives with a
/// release row already against its hash, and overwriting that with whatever the
/// lookup returns would be replacing known-good metadata with a guess.
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn already_identified(hash: &str, ctx: &ContentCtx) -> ContentResult<bool> {
    Ok(artifact_dao::get_release_by_hash(&ctx.db, hash)
        .await?
        .is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(project: &str) -> VersionDetail {
        VersionDetail {
            version_id: format!("{project}-v1"),
            project_id: project.into(),
            name: project.into(),
            version_number: "1.0.0".into(),
            changelog: None,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            published: chrono::Utc::now(),
            downloads: 0,
            files: Vec::new(),
            dependencies: Vec::new(),
        }
    }

    fn found(entries: &[(&str, &str)]) -> HashMap<String, (ProviderId, VersionDetail)> {
        entries
            .iter()
            .map(|(sha1, project)| {
                (
                    (*sha1).to_string(),
                    (ProviderId::Modrinth, version(project)),
                )
            })
            .collect()
    }

    #[test]
    fn every_answer_lands_on_the_file_it_is_about() {
        let identities = vec![
            Some(FileIdentity::from_sha1("aaa")),
            Some(FileIdentity::from_sha1("bbb")),
        ];

        let aligned = align(&identities, &found(&[("aaa", "sodium"), ("bbb", "iris")]));

        assert_eq!(aligned[0].as_ref().unwrap().version.project_id, "sodium");
        assert_eq!(aligned[1].as_ref().unwrap().version.project_id, "iris");
    }

    #[test]
    fn an_unreadable_file_does_not_shift_the_ones_after_it() {
        // The unreadable file never reached the provider, so the response is a
        // slot short. Zipping the two would hand the last file the answer meant
        // for the first.
        let identities = vec![
            None,
            Some(FileIdentity::from_sha1("aaa")),
            Some(FileIdentity::from_sha1("bbb")),
        ];

        let aligned = align(&identities, &found(&[("aaa", "sodium"), ("bbb", "iris")]));

        assert!(
            aligned[0].is_none(),
            "nothing is known about an unread file"
        );
        assert_eq!(aligned[1].as_ref().unwrap().version.project_id, "sodium");
        assert_eq!(aligned[2].as_ref().unwrap().version.project_id, "iris");
    }

    #[test]
    fn a_file_no_provider_claims_stays_local() {
        let identities = vec![Some(FileIdentity::from_sha1("ccc"))];

        let aligned = align(&identities, &found(&[("aaa", "sodium")]));

        assert!(aligned[0].is_none());
    }

    #[test]
    fn a_copy_is_named_the_way_an_installed_one_is() {
        // Matches `InstalledCopy::label`, which builds "<display_name>
        // <display_version>" out of the same two provider fields — the warning
        // puts the two side by side and they have to read alike.
        let identified = IdentifiedPackage {
            provider: ProviderId::Modrinth,
            version: version("Sodium"),
        };

        assert_eq!(identified.label(), "Sodium 1.0.0");
    }
}
