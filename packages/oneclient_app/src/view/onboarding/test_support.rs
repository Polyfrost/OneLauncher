use oneclient_core::clusters::{Cluster, ClusterStage};
use oneclient_core::packages::domain::{ContentType, GameLoader};
use oneclient_core::packages::types::ExternalFile;
use oneclient_core::{Bundle, BundleArchive, BundleFile, BundleFileKind, BundleManifest};
use std::path::PathBuf;

pub fn file(package_id: &str, enabled: bool, hidden: bool) -> BundleFile {
    BundleFile {
        enabled,
        hidden,
        path: format!("mods/{package_id}.jar"),
        size: 1,
        kind: BundleFileKind::External(ExternalFile {
            name: format!("{package_id}.jar"),
            url: format!("https://example.invalid/{package_id}.jar"),
            sha1: package_id.to_string(),
            size: 1,
            content_type: ContentType::Mod,
        }),
    }
}

pub fn archive(category: &str, enabled: bool, files: Vec<BundleFile>) -> BundleArchive {
    let name = format!("OneClient 1.21.11 Fabric [{category}]");
    BundleArchive {
        bundle: Bundle {
            remote_path: format!("/bundles/{category}.mrpack"),
            mc_version: "1.21.11".to_string(),
            loader: GameLoader::Fabric,
            file_name: format!("{category}.mrpack"),
            name: name.clone(),
            version_id: "1.0.0".to_string(),
            category: category.to_string(),
            loader_version: "0.16.0".to_string(),
            path: PathBuf::from("/tmp/unused.mrpack"),
            hidden: false,
        },
        manifest: BundleManifest {
            name,
            version_id: "1.0.0".to_string(),
            category: category.to_string(),
            mc_version: "1.21.11".to_string(),
            loader: GameLoader::Fabric,
            loader_version: "0.16.0".to_string(),
            enabled,
            files,
        },
    }
}

pub fn cluster(id: i64) -> Cluster {
    Cluster {
        id,
        name: format!("1.21.11 Fabric #{id}"),
        folder_name: format!("cluster-{id}"),
        setting_profile_name: None,
        mc_version: "1.21.11".to_string(),
        mc_loader: GameLoader::Fabric,
        mc_loader_version: None,
        stage: ClusterStage::default(),
        created_at: None,
        last_played: None,
        overall_played: std::time::Duration::ZERO,
        linked_modpack_hash: None,
    }
}

