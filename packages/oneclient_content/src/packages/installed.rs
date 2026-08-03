//! Whether a cluster already has something, asked before anything is added.
//!
//! Nothing here refuses an install. Duplicates are legal — `cluster_artifacts`
//! is unique on `(cluster_id, hash)`, and
//! [`reconcile_duplicate_activity`](super::reconcile_duplicate_activity) exists
//! precisely because a cluster can hold several versions of one package. This is
//! only the lookup a warning needs, kept in one place so every install path
//! asks the same question and gets the same answer.

use std::path::Path;

use oneclient_common::domain::{ContentType, ProviderId};
use polyio::{normalize_hash, sha1_file};

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::store::PackageStore;
use crate::packages::types::LinkedArtifactInfo;

/// A copy the cluster already links, reduced to what naming it in a warning
/// needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCopy {
    pub hash: String,
    /// The provider's version id, so a caller can tell "the same version again"
    /// from "a second version alongside the first". `None` for local files and
    /// for rows written before a provider release was recorded against them.
    pub version_id: Option<String>,
    /// How to name this copy to the user: the package and its version where the
    /// provider gave both, the file on disk otherwise.
    pub label: String,
}

impl From<LinkedArtifactInfo> for InstalledCopy {
    fn from(linked: LinkedArtifactInfo) -> Self {
        let label = match (&linked.display_name, &linked.display_version) {
            (Some(name), Some(version)) => format!("{name} {version}"),
            (Some(name), None) => name.clone(),
            _ => linked.cluster_file_name.clone(),
        };

        Self {
            hash: linked.hash,
            version_id: linked.version_id,
            label,
        }
    }
}

/// Every copy of a browsed package the cluster already holds.
///
/// Matched on the project rather than the version: a cluster that has Sodium
/// 0.5.3 already has Sodium, and installing 0.5.8 over it is the case worth
/// warning about.
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn installed_copies(
    provider: ProviderId,
    project_id: &str,
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<Vec<InstalledCopy>> {
    Ok(PackageStore::list_linked_artifacts(cluster_id, ctx)
        .await?
        .into_iter()
        .filter(|linked| {
            linked.provider == Some(provider) && linked.project_id.as_deref() == Some(project_id)
        })
        .map(InstalledCopy::from)
        .collect())
}

/// The cluster's existing copy of a file about to be imported, if it has one.
///
/// A local file has no project to match on, so this checks the two ways an
/// import can collide instead: the same bytes, which
/// [`PackageStore::import_local_file`] would relink rather than copy, and the
/// same name in the same folder, which is a second jar of what is almost
/// certainly the same mod. The name goes first so the usual answer costs no
/// read; the file is only hashed when the name says nothing.
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn installed_local_copy(
    path: &Path,
    content_type: ContentType,
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<Option<InstalledCopy>> {
    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;

    if let Some(name) = path.file_name().and_then(|name| name.to_str())
        && let Some(hit) = linked.iter().find(|l| matches_name(l, content_type, name))
    {
        return Ok(Some(hit.clone().into()));
    }

    let hash = normalize_hash(&sha1_file(path).await?);
    Ok(linked
        .into_iter()
        .find(|linked| linked.hash == hash)
        .map(InstalledCopy::from))
}

/// Scoped to the content type because the name is only a collision when the two
/// files would land in the same folder: `mods/x.jar` and `shaderpacks/x.jar` are
/// two different things that happen to share a name.
fn matches_name(linked: &LinkedArtifactInfo, content_type: ContentType, name: &str) -> bool {
    linked.content_type == content_type
        && (linked.cluster_file_name == name || linked.file_name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linked(display: Option<(&str, &str)>) -> LinkedArtifactInfo {
        LinkedArtifactInfo {
            hash: "abc".into(),
            cluster_file_name: "sodium-fabric-0.5.8.jar".into(),
            enabled: true,
            content_type: ContentType::Mod,
            file_name: "sodium-fabric-0.5.8.jar".into(),
            project_id: Some("sodium".into()),
            version_id: Some("v1".into()),
            display_name: display.map(|(name, _)| name.into()),
            display_version: display.map(|(_, version)| version.into()),
            provider: Some(ProviderId::Modrinth),
            published_at: None,
        }
    }

    #[test]
    fn a_copy_is_named_by_its_package_and_version() {
        let copy = InstalledCopy::from(linked(Some(("Sodium", "0.5.8"))));

        assert_eq!(copy.label, "Sodium 0.5.8");
    }

    #[test]
    fn a_copy_with_nothing_recorded_falls_back_to_the_file() {
        // The warning still has to name something. An artifact the provider
        // metadata never landed against is exactly the case where the user most
        // needs to be told which file is meant.
        let copy = InstalledCopy::from(linked(None));

        assert_eq!(copy.label, "sodium-fabric-0.5.8.jar");
    }

    #[test]
    fn a_name_only_collides_within_one_content_type() {
        let mod_file = linked(None);

        assert!(matches_name(
            &mod_file,
            ContentType::Mod,
            "sodium-fabric-0.5.8.jar"
        ));
        assert!(
            !matches_name(&mod_file, ContentType::Shader, "sodium-fabric-0.5.8.jar"),
            "two folders can hold the same name without colliding"
        );
    }
}
