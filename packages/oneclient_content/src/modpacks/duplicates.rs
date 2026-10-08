use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::DateTime;
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bundle as bundle_dao;

use crate::bundles::compare_version_like;
use crate::ctx::ContentCtx;
use crate::error::ContentResult;
use crate::packages::read_jar_manifest;
use crate::packages::store::{PackageStore, artifact_absolute_path};
use oneclient_common::domain::ContentType;

struct Copy {
    hash: String,
    name: String,
    version: Option<String>,
    published: Option<i64>,
    from_pack: bool,
    switchable: bool,
}

pub(super) struct DisabledCopy {
    pub hash: String,
    pub name: String,
    pub from_pack: bool,
}

fn newer(a: &Copy, b: &Copy) -> Option<Ordering> {
    if let (Some(left), Some(right)) = (&a.version, &b.version) {
        let by_version = compare_version_like(left, right);
        if by_version != Ordering::Equal {
            return Some(by_version);
        }
    }
    match (a.published, b.published) {
        (Some(left), Some(right)) => Some(left.cmp(&right)),
        _ if a.version.is_some() && b.version.is_some() => Some(Ordering::Equal),
        _ => None,
    }
}

fn keeper(copies: &[Copy]) -> Option<usize> {
    let mut best = 0;
    for index in 1..copies.len() {
        match newer(&copies[index], &copies[best])? {
            Ordering::Greater => best = index,
            Ordering::Equal if copies[best].from_pack && !copies[index].from_pack => best = index,
            _ => {}
        }
    }
    Some(best)
}

#[tracing::instrument(level = "debug", skip(ctx))]
pub(super) async fn disable_older_duplicates(
    cluster_id: i64,
    bundle_name: &str,
    ctx: &ContentCtx,
) -> ContentResult<Vec<DisabledCopy>> {
    let owners: HashMap<String, Option<String>> =
        bundle_dao::list_bundle_tracked(&ctx.db, cluster_id)
            .await?
            .into_iter()
            .map(|row| (row.hash, row.bundle_name))
            .collect();

    let mut groups: HashMap<String, Vec<Copy>> = HashMap::new();
    for info in PackageStore::list_linked_artifacts(cluster_id, ctx).await? {
        if !info.enabled || info.content_type != ContentType::Mod {
            continue;
        }
        let Some(row) = artifact_dao::get_artifact_by_hash(&ctx.db, &info.hash).await? else {
            continue;
        };
        let Ok(path) = artifact_absolute_path(&row.path) else {
            continue;
        };
        let manifest = read_jar_manifest(&path).await;
        let Some(mod_id) = manifest.mod_id else {
            continue;
        };

        let owner = owners.get(&info.hash).and_then(Option::as_deref);
        let from_pack = owner == Some(bundle_name);
        let switchable = from_pack || owner.is_none();
        groups.entry(mod_id.to_lowercase()).or_default().push(Copy {
            name: info
                .display_name
                .clone()
                .unwrap_or_else(|| info.file_name.clone()),
            published: info
                .published_at
                .as_deref()
                .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                .map(|at| at.timestamp()),
            version: manifest.version,
            hash: info.hash,
            from_pack,
            switchable,
        });
    }

    let mut disabled = Vec::new();
    for (mod_id, copies) in groups {
        if copies.len() < 2 || !copies.iter().any(|copy| copy.from_pack) {
            continue;
        }
        let Some(keep) = keeper(&copies) else {
            tracing::debug!(
                mod_id,
                "duplicate mod versions could not be compared, leaving them on"
            );
            continue;
        };
        for (index, copy) in copies.iter().enumerate() {
            if index == keep {
                continue;
            }
            if !copy.switchable {
                tracing::info!(mod_id, file = %copy.name, kept = %copies[keep].name, "older duplicate belongs to a bundle or another pack, leaving it on");
                continue;
            }
            PackageStore::set_artifact_enabled_to(cluster_id, &copy.hash, false, ctx).await?;
            tracing::info!(mod_id, file = %copy.name, kept = %copies[keep].name, "turned off an older duplicate mod");
            disabled.push(DisabledCopy {
                hash: copy.hash.clone(),
                name: copy.name.clone(),
                from_pack: copy.from_pack,
            });
        }
    }
    Ok(disabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy(version: Option<&str>, published: Option<i64>, from_pack: bool) -> Copy {
        Copy {
            hash: String::new(),
            name: String::new(),
            version: version.map(str::to_string),
            published,
            from_pack,
            switchable: true,
        }
    }

    #[test]
    fn the_newest_version_is_kept() {
        let copies = [
            copy(Some("0.8.9+mc26.2"), None, true),
            copy(Some("0.9.4+mc26.2"), None, false),
        ];
        assert_eq!(keeper(&copies), Some(1));
    }

    #[test]
    fn a_tie_keeps_the_copy_already_there() {
        let copies = [
            copy(Some("1.0"), None, true),
            copy(Some("1.0"), None, false),
        ];
        assert_eq!(keeper(&copies), Some(1));
    }

    #[test]
    fn missing_versions_fall_back_to_the_publish_date() {
        let copies = [
            copy(None, Some(20), true),
            copy(Some("2.0"), Some(10), false),
        ];
        assert_eq!(keeper(&copies), Some(0));
    }

    #[test]
    fn nothing_to_compare_changes_nothing() {
        let copies = [copy(None, None, true), copy(Some("2.0"), None, false)];
        assert_eq!(keeper(&copies), None);
    }
}
