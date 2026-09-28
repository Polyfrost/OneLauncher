//! `cluster_artifacts` is unique on `(cluster_id, hash)` so duplicate versions
//! of one package are legal rows
//! the newest stays enabled the rest are switched off rather than unlinked

use std::collections::{HashMap, HashSet};

use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_db::dao::applied_migration as migration_dao;
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::bundle as bundle_catalog_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;

use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::FileIdentity;
use crate::packages::dependencies::base_game_version;
use crate::packages::store::{PackageStore, record_release};
use crate::packages::types::LinkedArtifactInfo;

struct Copy {
    hash: String,
    enabled: bool,
    published_at: Option<String>,
}

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

/// Local files and a bundle's external files have no project to group on so
/// they are left out
/// two of those are two packages not two copies
fn group_duplicates(linked: &[LinkedArtifactInfo]) -> Vec<((ProviderId, String), Vec<Copy>)> {
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

#[tracing::instrument(level = "debug", skip(ctx))]
pub async fn disable_foreign_game_versions(
    cluster_id: i64,
    mc_version: &str,
    mc_loader: i64,
    ctx: &ContentCtx,
) -> ContentResult<Vec<String>> {
    if mc_version.is_empty() {
        return Ok(Vec::new());
    }

    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;

    // bundle installs skip the compatibility check so launch must trust them too
    let live_bundles: HashSet<String> =
        bundle_catalog_dao::list_visible_for_version_loader(&ctx.db, mc_version, mc_loader)
            .await?
            .into_iter()
            .filter_map(|row| row.name)
            .collect();

    let from_bundle: HashSet<String> = bundle_dao::list_bundle_tracked(&ctx.db, cluster_id)
        .await?
        .into_iter()
        .filter(|row| {
            row.bundle_name
                .as_deref()
                .is_some_and(|name| live_bundles.contains(name))
        })
        .map(|row| row.hash)
        .collect();

    let candidates: Vec<&LinkedArtifactInfo> = linked
        .iter()
        .filter(|info| {
            info.enabled
                && info.content_type == ContentType::Mod
                && !from_bundle.contains(&info.hash)
        })
        .collect();

    refresh_stated_game_versions(cluster_id, &candidates, mc_version, ctx).await;

    let mut switched_off = Vec::new();

    for info in candidates {
        match switch_off_if_foreign(cluster_id, info, mc_version, ctx).await {
            Ok(true) => switched_off.push(
                info.display_name
                    .clone()
                    .unwrap_or_else(|| info.file_name.clone()),
            ),
            Ok(false) => {}
            Err(err) => tracing::warn!(
                cluster_id,
                hash = %info.hash,
                %err,
                "could not switch off a mod built for another game version"
            ),
        }
    }

    Ok(switched_off)
}

const FOREIGN_VERSION_REPAIR: &str = "disable-foreign-game-version";

fn foreign_repair_id(cluster_id: i64, mc_version: &str, hash: &str) -> String {
    format!("{FOREIGN_VERSION_REPAIR}:{cluster_id}:{mc_version}:{hash}")
}

async fn refresh_stated_game_versions(
    cluster_id: i64,
    candidates: &[&LinkedArtifactInfo],
    mc_version: &str,
    ctx: &ContentCtx,
) {
    let mut stale = Vec::new();

    for info in candidates {
        match migration_dao::is_applied(
            &ctx.db,
            &foreign_repair_id(cluster_id, mc_version, &info.hash),
        )
        .await
        {
            Ok(true) => continue,
            Ok(false) => {}
            Err(err) => {
                tracing::debug!(hash = %info.hash, %err, "could not read the repair marker");
                continue;
            }
        }

        match artifact_dao::list_release_game_versions(&ctx.db, &info.hash).await {
            Ok(stated) if built_for_another_game_version(&stated, mc_version) => {
                stale.push(FileIdentity::from_sha1(&info.hash));
            }
            Ok(_) => {}
            Err(err) => {
                tracing::debug!(hash = %info.hash, %err, "could not read the stated game versions")
            }
        }
    }

    if stale.is_empty() {
        return;
    }

    let found = match ctx.providers.lookup_versions(&stale, ctx).await {
        Ok(found) => found,
        // Offline or the provider is down, so the stored rows are all there is
        Err(err) => {
            tracing::debug!(%err, "could not refresh the stated game versions");
            return;
        }
    };

    for (sha1, (provider, version)) in found {
        if let Err(err) = record_release(provider, &version, &sha1, ctx).await {
            tracing::debug!(hash = %sha1, %err, "could not store the refreshed game versions");
        }
    }
}

async fn switch_off_if_foreign(
    cluster_id: i64,
    info: &LinkedArtifactInfo,
    mc_version: &str,
    ctx: &ContentCtx,
) -> ContentResult<bool> {
    let repair_id = foreign_repair_id(cluster_id, mc_version, &info.hash);
    if migration_dao::is_applied(&ctx.db, &repair_id).await? {
        return Ok(false);
    }

    let stated = artifact_dao::list_release_game_versions(&ctx.db, &info.hash).await?;
    if !built_for_another_game_version(&stated, mc_version) {
        return Ok(false);
    }

    tracing::info!(
        cluster_id,
        hash = %info.hash,
        built_for = ?stated,
        "switching off a mod built for another game version"
    );

    PackageStore::set_artifact_enabled_to(cluster_id, &info.hash, false, ctx).await?;
    migration_dao::mark_applied(&ctx.db, &repair_id).await?;

    Ok(true)
}

fn built_for_another_game_version(stated: &[String], mc_version: &str) -> bool {
    !stated.is_empty()
        && !stated.iter().any(|versions| {
            let built_for: Vec<String> = serde_json::from_str(versions).unwrap_or_default();
            covers_game_version(&built_for, mc_version)
        })
}

fn covers_game_version(stated: &[String], mc_version: &str) -> bool {
    let wanted = base_game_version(mc_version);
    stated.is_empty()
        || stated.iter().any(|version| {
            let built_for = base_game_version(version);
            built_for == wanted
                || wanted
                    .strip_prefix(built_for)
                    .is_some_and(|rest| rest.starts_with('.'))
        })
}

/// `published_at` is RFC 3339 so string order matches time order
/// undated copies lose to dated ones and an all-undated group still picks one
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
    use oneclient_db::models::SeenStatus;

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
            seen_status: SeenStatus::Seen,
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

        assert_eq!(
            picked.as_deref(),
            Some("new"),
            "being enabled does not win it"
        );
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

        assert!(
            picked.is_some(),
            "a group with no dates must not go unresolved"
        );
    }

    #[test]
    fn only_a_stated_mismatch_switches_a_mod_off() {
        let foreign = |rows: &[&str]| {
            built_for_another_game_version(
                &rows.iter().map(|r| (*r).to_string()).collect::<Vec<_>>(),
                "26.1.2",
            )
        };

        assert!(
            !foreign(&[]),
            "a jar no provider ever described is left alone"
        );
        assert!(
            !foreign(&["[]"]),
            "an empty list is the provider saying nothing"
        );
        assert!(
            !foreign(&["[\"1.21.11"]),
            "a list that will not parse says nothing"
        );
        assert!(!foreign(&["[\"26.1.2\"]"]));
        assert!(
            !foreign(&["[\"1.21.11\",\"26.1.2\"]"]),
            "a multi-version build stays"
        );
        assert!(
            !foreign(&["[\"1.21.11\"]", "[\"26.1.2\"]"]),
            "one release row out of two is enough to keep it"
        );
        assert!(
            foreign(&["[\"1.21.11\"]"]),
            "the jars the old stash dragged in have to go"
        );
        assert!(
            !foreign(&["[\"26.1\"]"]),
            "a build for the release this cluster was migrated from stays"
        );
        assert!(
            foreign(&["[\"1.21.1\"]"]),
            "a shorter version that is not a component prefix is still foreign"
        );
    }

    #[test]
    fn a_pre_release_tag_is_the_same_game_version() {
        let foreign = |rows: &[&str], mc_version: &str| {
            built_for_another_game_version(
                &rows.iter().map(|r| (*r).to_string()).collect::<Vec<_>>(),
                mc_version,
            )
        };

        assert!(
            !foreign(&["[\"26.3-rc-1\"]"], "26.3"),
            "a jar built against the release candidate runs on the release"
        );
        assert!(!foreign(&["[\"26.3-snapshot-7\",\"26.3-pre-2\"]"], "26.3"));
        assert!(
            !foreign(&["[\"26.3\"]"], "26.3-snapshot-10"),
            "and a snapshot cluster keeps the release build"
        );
        assert!(
            !foreign(&["[\"26.3-rc-1\"]"], "26.3.1"),
            "the component prefix still applies once the tag is off"
        );
        assert!(
            foreign(&["[\"26.2-rc-1\"]"], "26.3"),
            "stripping the tag must not blur two game versions together"
        );
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
            info(
                Some("sodium"),
                "newest",
                false,
                Some("2026-06-01T00:00:00Z"),
            ),
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
