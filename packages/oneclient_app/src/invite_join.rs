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
    },
    Unknown,
}

#[must_use]
pub fn plan(game: &InviteGame, candidates: Vec<i64>, oneclient: bool) -> InviteJoin {
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
pub fn custom_loader(loader: GameLoader) -> bool {
    !matches!(loader, GameLoader::Quilt | GameLoader::Ornithe)
}

#[must_use]
pub fn has_polyplus(artifacts: &[LinkedArtifactInfo]) -> bool {
    artifacts.iter().any(|artifact| {
        artifact.enabled
            && artifact.provider == Some(POLYPLUS.provider)
            && artifact.project_id.as_deref() == Some(POLYPLUS.project_id)
    })
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
        assert_eq!(plan(&known(), vec![4], true), InviteJoin::Launch(4));
        assert_eq!(plan(&known(), vec![4, 5], true), InviteJoin::Pick(vec![4, 5]));
    }

    #[test]
    fn no_match_offers_to_create_and_remembers_support() {
        assert_eq!(
            plan(&known(), Vec::new(), true),
            InviteJoin::Create {
                version: "1.21.1".to_string(),
                loader: GameLoader::Fabric,
                oneclient: true,
            }
        );
    }

    #[test]
    fn an_unknown_game_never_launches_on_its_own() {
        let unknown = InviteGame::default();

        assert_eq!(plan(&unknown, vec![4], true), InviteJoin::Pick(vec![4]));
        assert_eq!(plan(&unknown, Vec::new(), true), InviteJoin::Unknown);
    }

    #[test]
    fn half_known_games_count_as_unknown() {
        let game = InviteGame {
            version: Some("1.21.1".to_string()),
            loader: None,
        };

        assert_eq!(plan(&game, vec![4], true), InviteJoin::Pick(vec![4]));
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
        assert!(!oneclient_supports(&catalogue, "1.21.2", GameLoader::Fabric));
    }

    #[test]
    fn the_join_argument_carries_the_invite() {
        assert_eq!(join_jvm_arg(38), "-Dpolyplus.joinInvite=38");
    }
}
