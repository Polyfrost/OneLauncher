use oneclient_common::GameLoader;
use oneclient_core::LinkedArtifactInfo;
use oneclient_core::versions::VersionMetadata;

use crate::chat::InviteGame;
use crate::essential::POLYPLUS;

#[must_use]
pub fn join_jvm_arg(invite_id: i32) -> String {
    format!("-Dpolyplus.joinInvite={invite_id}")
}

#[derive(Clone, Debug, PartialEq)]
pub enum InviteJoin {
    Launch(i64),
    Pick(Vec<i64>),
    Create {
        version: String,
        loader: GameLoader,
        oneclient: bool,
        unlinked: bool,
    },
    Unknown,
}

#[must_use]
pub fn plan(
    game: &InviteGame,
    candidates: Vec<i64>,
    unlinked: bool,
    oneclient: bool,
) -> InviteJoin {
    let Some((version, loader)) = game.known() else {
        return if candidates.is_empty() {
            InviteJoin::Unknown
        } else {
            InviteJoin::Pick(candidates)
        };
    };

    match candidates.as_slice() {
        [] => InviteJoin::Create {
            version: version.to_string(),
            loader,
            oneclient,
            unlinked,
        },
        [only] => InviteJoin::Launch(*only),
        _ => InviteJoin::Pick(candidates),
    }
}

#[must_use]
pub fn cluster_matches(game: &InviteGame, version: &str, loader: GameLoader) -> bool {
    game.known()
        .is_none_or(|(wanted, wanted_loader)| wanted == version && wanted_loader == loader)
}

#[must_use]
pub fn wizard_invite(
    invite: Option<i32>,
    prefill: Option<&(String, GameLoader)>,
    version: Option<&str>,
    loader: Option<GameLoader>,
) -> Option<i32> {
    invite.filter(|_| {
        prefill.is_some_and(|(wanted, wanted_loader)| {
            version == Some(wanted.as_str()) && loader == Some(*wanted_loader)
        })
    })
}

#[must_use]
pub fn custom_loader(loader: GameLoader) -> bool {
    loader != GameLoader::Quilt
}

#[must_use]
pub fn has_polyplus(artifacts: &[LinkedArtifactInfo], local: &[String]) -> bool {
    artifacts
        .iter()
        .any(|artifact| artifact.enabled && is_polyplus(artifact, local))
}

#[must_use]
pub fn disabled_polyplus<'a>(
    artifacts: &'a [LinkedArtifactInfo],
    local: &[String],
) -> Option<&'a str> {
    artifacts
        .iter()
        .find(|artifact| !artifact.enabled && is_polyplus(artifact, local))
        .map(|artifact| artifact.hash.as_str())
}

pub const POLYPLUS_MOD_ID: &str = "polyplus";

#[must_use]
pub fn maybe_local(artifact: &LinkedArtifactInfo) -> bool {
    artifact.content_type == oneclient_common::ContentType::Mod
        && matches!(
            artifact.provider,
            None | Some(oneclient_common::ProviderId::Local)
        )
}

fn is_polyplus(artifact: &LinkedArtifactInfo, local: &[String]) -> bool {
    (artifact.provider == Some(POLYPLUS.provider)
        && artifact.project_id.as_deref() == Some(POLYPLUS.project_id))
        || local.contains(&artifact.hash)
}

#[must_use]
pub fn oneclient_supports(metadata: &[VersionMetadata], version: &str, loader: GameLoader) -> bool {
    metadata.iter().any(|entry| {
        entry.mc_version().as_deref() == Some(version)
            && entry
                .loader
                .as_deref()
                .and_then(|name| name.parse::<GameLoader>().ok())
                == Some(loader)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> InviteGame {
        InviteGame {
            version: Some("1.21.1".to_string()),
            loader: Some(GameLoader::Fabric),
        }
    }

    fn entry(minor: u32, patch: Option<u32>, loader: &str) -> VersionMetadata {
        VersionMetadata {
            major_version: 21,
            minor_version: Some(minor),
            patch_version: patch,
            loader: Some(loader.to_string()),
            name: String::new(),
            art_url: None,
            long_description: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn one_match_launches_and_several_ask() {
        assert_eq!(plan(&known(), vec![4], false, true), InviteJoin::Launch(4));
        assert_eq!(
            plan(&known(), vec![4, 5], false, true),
            InviteJoin::Pick(vec![4, 5])
        );
    }

    #[test]
    fn no_match_offers_to_create_and_remembers_support() {
        assert_eq!(
            plan(&known(), Vec::new(), false, true),
            InviteJoin::Create {
                version: "1.21.1".to_string(),
                loader: GameLoader::Fabric,
                oneclient: true,
                unlinked: false,
            }
        );
    }

    #[test]
    fn an_unknown_game_never_launches_on_its_own() {
        let unknown = InviteGame::default();

        assert_eq!(
            plan(&unknown, vec![4], false, true),
            InviteJoin::Pick(vec![4])
        );
        assert_eq!(plan(&unknown, Vec::new(), false, true), InviteJoin::Unknown);
    }

    #[test]
    fn half_known_games_count_as_unknown() {
        let game = InviteGame {
            version: Some("1.21.1".to_string()),
            loader: None,
        };

        assert_eq!(plan(&game, vec![4], false, true), InviteJoin::Pick(vec![4]));
    }

    #[test]
    fn a_matching_cluster_without_polyplus_is_offered_for_reuse() {
        assert!(matches!(
            plan(&known(), Vec::new(), true, true),
            InviteJoin::Create { unlinked: true, .. }
        ));
    }

    #[test]
    fn the_wizard_only_joins_when_it_still_makes_the_invited_game() {
        let prefill = ("1.21.1".to_string(), GameLoader::Fabric);
        let wanted = Some(&prefill);

        assert_eq!(
            wizard_invite(Some(3), wanted, Some("1.21.1"), Some(GameLoader::Fabric)),
            Some(3)
        );
        assert_eq!(
            wizard_invite(Some(3), wanted, Some("1.20.1"), Some(GameLoader::Fabric)),
            None
        );
        assert_eq!(
            wizard_invite(Some(3), wanted, Some("1.21.1"), Some(GameLoader::Forge)),
            None
        );
        assert_eq!(
            wizard_invite(None, wanted, Some("1.21.1"), Some(GameLoader::Fabric)),
            None
        );
    }

    #[test]
    fn a_custom_cluster_is_offered_for_every_loader_the_wizard_keeps() {
        assert!(custom_loader(GameLoader::Ornithe));
        assert!(custom_loader(GameLoader::Fabric));
        assert!(!custom_loader(GameLoader::Quilt));
    }

    #[test]
    fn matching_needs_the_exact_version_and_loader() {
        assert!(cluster_matches(&known(), "1.21.1", GameLoader::Fabric));
        assert!(!cluster_matches(&known(), "1.21", GameLoader::Fabric));
        assert!(!cluster_matches(&known(), "1.21.1", GameLoader::Quilt));
        assert!(cluster_matches(
            &InviteGame::default(),
            "1.8.9",
            GameLoader::Forge
        ));
    }

    #[test]
    fn support_needs_the_exact_row_in_the_catalogue() {
        let catalogue = [entry(1, None, "fabric")];

        assert!(oneclient_supports(&catalogue, "1.21.1", GameLoader::Fabric));
        assert!(!oneclient_supports(&catalogue, "1.21.1", GameLoader::Forge));
        assert!(!oneclient_supports(
            &catalogue,
            "1.21.2",
            GameLoader::Fabric
        ));
    }

    fn artifact(hash: &str, enabled: bool, project: &str) -> LinkedArtifactInfo {
        LinkedArtifactInfo {
            hash: hash.to_string(),
            cluster_file_name: String::new(),
            enabled,
            content_type: oneclient_common::ContentType::Mod,
            file_name: String::new(),
            size_bytes: None,
            project_id: Some(project.to_string()),
            version_id: None,
            display_name: None,
            display_version: None,
            provider: Some(POLYPLUS.provider),
            published_at: None,
            added_at: None,
            seen_status: oneclient_core::SeenStatus::Seen,
        }
    }

    #[test]
    fn a_disabled_polyplus_is_enabled_instead_of_installed_again() {
        let disabled = [
            artifact("other", false, "else"),
            artifact("plus", false, POLYPLUS.project_id),
        ];
        assert!(!has_polyplus(&disabled, &[]));
        assert_eq!(disabled_polyplus(&disabled, &[]), Some("plus"));

        let enabled = [artifact("plus", true, POLYPLUS.project_id)];
        assert!(has_polyplus(&enabled, &[]));
        assert_eq!(disabled_polyplus(&enabled, &[]), None);

        assert_eq!(
            disabled_polyplus(&[artifact("other", false, "else")], &[]),
            None
        );
    }

    #[test]
    fn a_local_polyplus_counts_too() {
        let mut local = artifact("dev", true, "dev");
        local.provider = Some(oneclient_common::ProviderId::Local);
        let hashes = ["dev".to_string()];

        assert!(maybe_local(&local));
        assert!(!has_polyplus(std::slice::from_ref(&local), &[]));
        assert!(has_polyplus(std::slice::from_ref(&local), &hashes));

        local.enabled = false;
        assert_eq!(
            disabled_polyplus(std::slice::from_ref(&local), &hashes),
            Some("dev")
        );
    }

    #[test]
    fn the_join_argument_carries_the_invite() {
        assert_eq!(join_jvm_arg(38), "-Dpolyplus.joinInvite=38");
    }
}
