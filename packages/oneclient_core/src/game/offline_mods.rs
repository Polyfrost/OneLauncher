//! Mods that only make sense with a real Microsoft session.
//!
//! The default bundle ships **PolyPlus**, which calls Mojang's `joinServer`,
//! opens a Poly+ websocket and retries both a few seconds apart. On an offline
//! account every attempt fails: the game still starts and plays, but the log
//! fills with `Failed to authenticate with Mojang` and `WebSocket still
//! rejected (401)`, and the extra requests slow the launch down.
//!
//! The mods folder is manifest-managed, so deleting the jar would just be undone
//! by the next bundle install. Disabling the artifact through the package store
//! is the supported way: it survives bundle updates and is reversible.

use oneclient_auth::MinecraftAccount;
use oneclient_common::domain::ContentType;
use oneclient_content::ContentCtx;
use oneclient_content::packages::PackageStore;

use crate::LauncherResult;

/// Lowercase file-name prefixes of mods that need a Microsoft session.
const MICROSOFT_ONLY_MODS: &[&str] = &["polyplus"];

/// Whether this jar can only work for a signed-in Microsoft account.
#[must_use]
pub fn is_microsoft_only(file_name: &str) -> bool {
    let name = file_name.to_ascii_lowercase();
    MICROSOFT_ONLY_MODS
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Turns Microsoft-only mods off for an offline account and back on for a
/// Microsoft one. Returns how many artifacts were toggled.
pub async fn sync_for_account(
    cluster_id: i64,
    account: &MinecraftAccount,
    ctx: &ContentCtx,
) -> LauncherResult<usize> {
    let want_enabled = !account.is_offline();
    let linked = PackageStore::list_linked_artifacts(cluster_id, ctx).await?;
    let mut toggled = 0;

    for info in linked {
        if info.content_type != ContentType::Mod
            || info.enabled == want_enabled
            || !is_microsoft_only(&info.file_name)
        {
            continue;
        }
        match PackageStore::set_artifact_enabled_to(cluster_id, &info.hash, want_enabled, ctx).await {
            Ok(_) => toggled += 1,
            Err(err) => tracing::warn!(
                cluster_id,
                mod_file = %info.file_name,
                error = %err,
                "could not toggle a Microsoft-only mod; leaving it as it is"
            ),
        }
    }

    if toggled > 0 {
        tracing::info!(
            cluster_id,
            toggled,
            enabled = want_enabled,
            "adjusted mods that need a Microsoft session"
        );
    }
    Ok(toggled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_polyplus_jar_is_microsoft_only() {
        assert!(is_microsoft_only("polyplus-1.4.7+26.3.jar"));
        assert!(is_microsoft_only("PolyPlus.jar"));
    }

    #[test]
    fn other_poly_mods_are_left_alone() {
        for name in [
            "polyblur-2.2.8+26.3.jar",
            "polysprint-1.3.3+26.3.jar",
            "OneConfig-26.3-fabric-1.6.2.jar",
            "sodium-fabric-0.6.0.jar",
        ] {
            assert!(!is_microsoft_only(name), "{name} must stay enabled");
        }
    }
}
