//! Which copy wins when a cluster holds several versions of one package.
//!
//! Holding more than one is a defect the launcher does not aim for, but nothing
//! in the schema forbids it: `cluster_artifacts` is unique on `(cluster_id,
//! hash)`, so two versions are two perfectly legal rows. Until that changes, a
//! cluster can reach a state where every copy is enabled and the game is handed
//! three jars of the same mod — which for mods is a classloader conflict, not a
//! preference.
//!
//! The rule here is that the newest copy is the live one and the rest are
//! switched off. Nothing is unlinked: the older versions stay installed, stay
//! visible in the package manager, and can be switched back on one at a time.
//! `enabled` is also what materialization already reads, so enforcing it here
//! needs no special case at launch.

use std::collections::HashMap;

use oneclient_common::domain::ProviderId;

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::store::PackageStore;
use crate::packages::types::LinkedArtifactInfo;

/// One copy of a package, reduced to what picking a winner needs.
struct Copy {
    hash: String,
    enabled: bool,
    published_at: Option<String>,
}

/// Leaves each project in the cluster with the newest copy.
///
/// A no-op on a cluster with no duplicates, which is every healthy one, so it is
/// cheap enough to run before a launch and after an install.
#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn reconcile_duplicate_activity(
    cluster_id: i64,
    ctx: &ContentCtx,
) -> ContentResult<Vec<String>> {
    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
    let mut switched_off = Vec::new();

    for hash in duplicates_to_disable(&linked) {
        tracing::info!(
            cluster_id,
            %hash,
            "resolving duplicate versions of a package to the newest live one"
        );

        PackageStore::set_artifact_enabled_to(cluster_id, &hash, false, ctx).await?;
        switched_off.push(hash);
    }

    Ok(switched_off)
}

fn duplicates_to_disable(linked: &[LinkedArtifactInfo]) -> Vec<String> {
    let mut out = Vec::new();

    for (_, copies) in group_duplicates(linked) {
        let live: Vec<&Copy> = copies.iter().filter(|copy| copy.enabled).collect();
        if live.len() < 2 {
            continue;
        }

        let Some(winner) = newest(live.iter().copied()) else {
            continue;
        };

        out.extend(
            live.iter()
                .filter(|copy| copy.hash != winner)
                .map(|copy| copy.hash.clone()),
        );
    }

    out
}

/// Groups the cluster's content by project, keeping only the projects it has
/// more than one copy of.
///
/// Local files and a bundle's external files have no project to group on and
/// are left out; two of those are two different packages as far as anything
/// here can tell.
fn group_duplicates(
    linked: &[LinkedArtifactInfo],
) -> Vec<((ProviderId, String), Vec<Copy>)> {
    let mut by_project: HashMap<(ProviderId, String), Vec<Copy>> = HashMap::new();

    for info in linked {
        let (Some(provider), Some(project_id)) = (info.provider, info.project_id.clone()) else {
            continue;
        };

        by_project
            .entry((provider, project_id))
            .or_default()
            .push(Copy {
                hash: info.hash.clone(),
                enabled: info.enabled,
                published_at: info.published_at.clone(),
            });
    }

    by_project.retain(|_, copies| copies.len() > 1);
    by_project.into_iter().collect()
}

/// The hash of the newest copy.
///
/// `published_at` is written as RFC 3339 by the download path, so the strings
/// order the same way the timestamps do. A copy the provider gave no date for
/// loses to any copy that has one — it is usually an older row written before
/// the field was recorded — and an all-undated group falls back to the first,
/// which at least makes the choice stable across runs rather than arbitrary.
fn newest<'a>(copies: impl IntoIterator<Item = &'a Copy>) -> Option<String> {
    copies
        .into_iter()
        .max_by(|a, b| a.published_at.cmp(&b.published_at))
        .map(|copy| copy.hash.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneclient_common::domain::ContentType;

    fn info(
        project_id: Option<&str>,
        hash: &str,
        enabled: bool,
        published_at: Option<&str>,
    ) -> LinkedArtifactInfo {
        LinkedArtifactInfo {
            hash: hash.into(),
            cluster_file_name: format!("{hash}.jar"),
            enabled,
            content_type: ContentType::Mod,
            file_name: format!("{hash}.jar"),
            project_id: project_id.map(Into::into),
            version_id: Some(format!("v-{hash}")),
            display_name: None,
            display_version: None,
            provider: project_id.map(|_| ProviderId::Modrinth),
            published_at: published_at.map(Into::into),
        }
    }

    fn copies(specs: &[(&str, bool, Option<&str>)]) -> Vec<Copy> {
        specs
            .iter()
            .map(|(hash, enabled, published)| Copy {
                hash: (*hash).into(),
                enabled: *enabled,
                published_at: published.map(Into::into),
            })
            .collect()
    }

    #[test]
    fn the_newest_publication_wins() {
        let picked = newest(&copies(&[
            ("old", true, Some("2026-01-01T00:00:00Z")),
            ("new", false, Some("2026-06-01T00:00:00Z")),
            ("middle", false, Some("2026-03-01T00:00:00Z")),
        ]));

        assert_eq!(picked.as_deref(), Some("new"), "being enabled does not win it");
    }

    #[test]
    fn a_dated_copy_beats_an_undated_one() {
        let picked = newest(&copies(&[
            ("undated", true, None),
            ("dated", false, Some("2026-01-01T00:00:00Z")),
        ]));

        assert_eq!(picked.as_deref(), Some("dated"));
    }

    #[test]
    fn an_undated_group_still_picks_one() {
        let picked = newest(&copies(&[("a", false, None), ("b", false, None)]));

        assert!(picked.is_some(), "a group with no dates must not go unresolved");
    }

    #[test]
    fn only_projects_with_several_copies_are_reconciled() {
        let groups = group_duplicates(&[
            info(Some("sodium"), "s1", true, None),
            info(Some("sodium"), "s2", true, None),
            info(Some("lithium"), "l1", true, None),
        ]);

        assert_eq!(groups.len(), 1, "a single copy is nothing to resolve");
        assert_eq!(groups[0].0.1, "sodium");
        assert_eq!(groups[0].1.len(), 2);
    }

    #[test]
    fn the_newest_live_copy_survives_and_the_rest_go_quiet() {
        let mut disable = duplicates_to_disable(&[
            info(Some("sodium"), "old", true, Some("2026-01-01T00:00:00Z")),
            info(Some("sodium"), "new", true, Some("2026-06-01T00:00:00Z")),
            info(Some("sodium"), "middle", true, Some("2026-03-01T00:00:00Z")),
        ]);
        disable.sort();
        assert_eq!(disable, vec!["middle".to_string(), "old".to_string()]);
    }

    #[test]
    fn a_copy_that_is_already_off_is_not_counted_as_a_conflict() {
        let disable = duplicates_to_disable(&[
            info(Some("sodium"), "live", true, Some("2026-01-01T00:00:00Z")),
            info(Some("sodium"), "off", false, Some("2026-06-01T00:00:00Z")),
        ]);

        assert!(
            disable.is_empty(),
            "one live copy is not a duplicate, however many switched-off copies sit beside it"
        );
    }

    #[test]
    fn the_user_disabling_the_newest_copy_is_left_standing() {
        let disable = duplicates_to_disable(&[
            info(Some("sodium"), "chosen", true, Some("2026-01-01T00:00:00Z")),
            info(Some("sodium"), "newest", false, Some("2026-06-01T00:00:00Z")),
        ]);

        assert!(
            disable.is_empty(),
            "picking the older version on purpose must not be undone, and the newest must not be \
             switched back on"
        );
    }

    #[test]
    fn a_project_with_no_live_copy_at_all_is_left_alone() {
        let disable = duplicates_to_disable(&[
            info(Some("sodium"), "a", false, Some("2026-01-01T00:00:00Z")),
            info(Some("sodium"), "b", false, Some("2026-06-01T00:00:00Z")),
        ]);

        assert!(disable.is_empty());
    }

    #[test]
    fn content_with_no_project_is_left_alone() {
        let groups = group_duplicates(&[
            info(None, "local-1", true, None),
            info(None, "local-2", true, None),
        ]);

        assert!(
            groups.is_empty(),
            "two local files are two packages, not two copies of one"
        );
    }
}
