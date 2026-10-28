#![recursion_limit = "256"]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_content::packages::PackageStore;
use oneclient_core::LauncherState;
use oneclient_core::clusters::{ClusterKind, CreateClusterOptions};
use oneclient_core::sync::{
    CloudSync, LocalSyncStore, Resolution, SyncAccount, SyncLedger, SyncOutcome, SyncStatus,
    SyncStore,
};
use uuid::Uuid;

fn jar(tag: &str) -> Vec<u8> {
    let mut bytes = b"PK\x05\x06".to_vec();
    bytes.extend([0; 16]);
    bytes.extend((tag.len() as u16).to_le_bytes());
    bytes.extend(tag.as_bytes());
    bytes
}

struct Machine {
    state: Arc<LauncherState>,
    sync: CloudSync,
}

impl Machine {
    async fn new(scratch: &Path, cloud: &Path, name: &str) -> Self {
        let state = oneclient_core::dev::ephemeral_state().await.unwrap();
        let dir = scratch.join(name);
        let sync = CloudSync::new(
            Arc::new(LocalSyncStore::new(cloud)),
            SyncLedger::at(dir.join("state.json")),
            name,
            dir.join("incoming"),
        );
        Self { state, sync }
    }

    async fn game_dir(&self, cluster_id: i64) -> PathBuf {
        self.state
            .clusters
            .get(cluster_id)
            .await
            .unwrap()
            .game_dir()
            .unwrap()
    }

    async fn write(&self, cluster_id: i64, file: &str, bytes: &[u8]) {
        let path = self.game_dir(cluster_id).await.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    async fn read(&self, cluster_id: i64, file: &str) -> Vec<u8> {
        std::fs::read(self.game_dir(cluster_id).await.join(file)).unwrap()
    }

    async fn status(&self, account: &SyncAccount, cluster_id: i64) -> Option<SyncStatus> {
        self.sync
            .statuses(account)
            .await
            .unwrap()
            .get(&cluster_id)
            .cloned()
    }

    async fn mods(&self, cluster_id: i64) -> Vec<String> {
        let ctx = self.state.services.content();
        let mut names: Vec<String> = PackageStore::list_linked_artifacts(cluster_id, &ctx)
            .await
            .unwrap()
            .into_iter()
            .filter(|link| link.content_type == ContentType::Mod)
            .map(|link| link.cluster_file_name)
            .collect();
        names.sort();
        names
    }
}

async fn create_instance(machine: &Machine, name: &str) -> i64 {
    let global = machine.state.settings.read().global_game_settings.clone();
    let mut options = CreateClusterOptions::new(name, "1.21.1", GameLoader::Fabric)
        .kind(ClusterKind::Modded)
        .user_created(true);
    options.mc_loader_version = Some("0.16.5".into());
    machine
        .state
        .clusters
        .create(&global, options)
        .await
        .unwrap()
        .id
}

async fn add_jar(machine: &Machine, scratch: &Path, cluster_id: i64, name: &str) {
    let path = scratch.join(name);
    std::fs::write(&path, jar(name)).unwrap();
    PackageStore::import_local_files(
        &[(path, ContentType::Mod)],
        cluster_id,
        &machine.state.services.content(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn the_local_store_checks_revisions() {
    let scratch = polyio::testing::ScratchDir::new("sync-store-revisions");
    let store = LocalSyncStore::new(scratch.path().join("cloud"));
    let account = SyncAccount {
        id: Uuid::new_v4(),
        name: "Steve".into(),
        token: None,
    };
    let payload = scratch.path().join("a.mrpack");
    std::fs::write(&payload, b"one").unwrap();
    let meta = oneclient_core::sharing::ShareMeta {
        format: 1,
        name: "Pack".into(),
        mc_version: "1.21.1".into(),
        mc_loader: GameLoader::Fabric,
        kind: ClusterKind::Modded,
        size: 3,
        sha512: "00".into(),
        created_at: None,
    };
    let upload = oneclient_core::sync::SyncUpload {
        meta: &meta,
        provisioned: false,
        device: "test",
    };

    let created = store.create(&account, &payload, upload).await.unwrap();
    assert_eq!(created.revision, 1);
    let updated = store
        .put(&account, &created.id, 1, &payload, upload)
        .await
        .unwrap();
    assert_eq!(updated.revision, 2);
    assert!(matches!(
        store.put(&account, &created.id, 1, &payload, upload).await,
        Err(oneclient_core::sync::SyncError::Conflict { revision: 2, .. })
    ));

    let stranger = SyncAccount {
        id: Uuid::new_v4(),
        name: "Alex".into(),
        token: None,
    };
    assert!(store.list(&stranger).await.unwrap().is_empty());
    assert_eq!(store.list(&account).await.unwrap().len(), 1);

    store.delete(&account, &created.id, Some(2)).await.unwrap();
    let listed = store.list(&account).await.unwrap();
    assert!(listed[0].deleted);
    assert!(matches!(
        store
            .download(&account, &created.id, &scratch.path().join("out.mrpack"))
            .await,
        Err(oneclient_core::sync::SyncError::Deleted(_))
    ));
}

#[test]
fn instances_sync_between_two_machines() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(two_machines());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn two_machines() {
    let scratch = polyio::testing::ScratchDir::new("sync-two-machines");
    let cloud = scratch.path().join("cloud");
    let a = Machine::new(scratch.path(), &cloud, "machine-a").await;
    let b = Machine::new(scratch.path(), &cloud, "machine-b").await;
    let account = SyncAccount {
        id: Uuid::new_v4(),
        name: "Steve".into(),
        token: None,
    };

    let on_a = create_instance(&a, "Sync Pack").await;
    add_jar(&a, scratch.path(), on_a, "first.jar").await;
    a.write(on_a, "options.txt", b"fov:0.5").await;
    a.write(on_a, "config/polysprint/main.json", b"{\"on\":true}")
        .await;
    a.write(on_a, "config/session-token.json", b"secret").await;
    a.write(on_a, "saves/World A/level.dat", b"a-world").await;

    assert_eq!(
        a.sync.enable(&a.state, &account, on_a).await.unwrap(),
        SyncOutcome::Pushed { revision: 1 }
    );
    assert_eq!(
        a.sync.sync_cluster(&a.state, &account, on_a).await.unwrap(),
        SyncOutcome::Unchanged,
        "an untouched instance exports the same way twice"
    );

    let report = b.sync.sync_all(&b.state, &account).await.unwrap();
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert_eq!(report.created.len(), 1);
    let on_b = report.created[0];
    let pulled = b.state.clusters.get(on_b).await.unwrap();
    assert_eq!(pulled.name, "Sync Pack");
    assert_eq!(pulled.mc_loader, GameLoader::Fabric);
    assert_eq!(pulled.mc_loader_version.as_deref(), Some("0.16.5"));
    assert_eq!(b.mods(on_b).await, vec!["first.jar".to_string()]);
    assert_eq!(b.read(on_b, "options.txt").await, b"fov:0.5");
    assert_eq!(
        b.read(on_b, "config/polysprint/main.json").await,
        b"{\"on\":true}"
    );
    let b_dir = b.game_dir(on_b).await;
    assert!(!b_dir.join("config/session-token.json").exists());
    assert!(!b_dir.join("saves/World A").exists());
    assert_eq!(b.status(&account, on_b).await, Some(SyncStatus::Synced));
    assert_eq!(
        b.sync.sync_cluster(&b.state, &account, on_b).await.unwrap(),
        SyncOutcome::Unchanged
    );

    b.write(on_b, "saves/World B/level.dat", b"b-world").await;
    a.write(on_a, "config/polysprint/main.json", b"{\"on\":false}")
        .await;
    add_jar(&a, scratch.path(), on_a, "second.jar").await;
    assert_eq!(
        a.sync.sync_cluster(&a.state, &account, on_a).await.unwrap(),
        SyncOutcome::Pushed { revision: 2 }
    );
    let report = b.sync.sync_all(&b.state, &account).await.unwrap();
    assert_eq!(report.pulled, vec![on_b]);
    assert!(report.created.is_empty());
    assert_eq!(
        b.mods(on_b).await,
        vec!["first.jar".to_string(), "second.jar".to_string()]
    );
    assert_eq!(
        b.read(on_b, "config/polysprint/main.json").await,
        b"{\"on\":false}"
    );
    assert_eq!(b.read(on_b, "saves/World B/level.dat").await, b"b-world");

    a.write(on_a, "options.txt", b"fov:a").await;
    b.write(on_b, "options.txt", b"fov:b").await;
    assert_eq!(
        a.sync.sync_cluster(&a.state, &account, on_a).await.unwrap(),
        SyncOutcome::Pushed { revision: 3 }
    );
    assert_eq!(
        b.sync.sync_cluster(&b.state, &account, on_b).await.unwrap(),
        SyncOutcome::Conflict { revision: 3 }
    );
    assert_eq!(
        b.status(&account, on_b).await,
        Some(SyncStatus::Conflict { revision: 3 })
    );
    assert_eq!(b.read(on_b, "options.txt").await, b"fov:b");

    let SyncOutcome::Copied {
        copy_cluster_id: copy,
    } = b
        .sync
        .resolve(&b.state, &account, on_b, Resolution::KeepBoth)
        .await
        .unwrap()
    else {
        panic!("keeping both makes a copy");
    };
    assert_ne!(copy, on_b);
    assert_eq!(b.read(on_b, "options.txt").await, b"fov:b");
    assert_eq!(b.read(copy, "options.txt").await, b"fov:a");
    assert_eq!(b.status(&account, on_b).await, Some(SyncStatus::Synced));
    assert_eq!(b.status(&account, copy).await, Some(SyncStatus::Synced));

    let report = a.sync.sync_all(&a.state, &account).await.unwrap();
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert!(report.pulled.is_empty());
    assert_eq!(report.created.len(), 1, "B's copy shows up on A");
    let b_copy_on_a = report.created[0];
    assert_eq!(a.read(b_copy_on_a, "options.txt").await, b"fov:b");
    assert_eq!(a.read(on_a, "options.txt").await, b"fov:a");

    a.write(on_a, "options.txt", b"fov:a2").await;
    b.write(copy, "options.txt", b"fov:b2").await;
    a.sync.sync_cluster(&a.state, &account, on_a).await.unwrap();
    assert!(matches!(
        b.sync.sync_cluster(&b.state, &account, copy).await.unwrap(),
        SyncOutcome::Conflict { .. }
    ));
    assert!(matches!(
        b.sync
            .resolve(&b.state, &account, copy, Resolution::KeepCloud)
            .await
            .unwrap(),
        SyncOutcome::Pulled { .. }
    ));
    assert_eq!(b.read(copy, "options.txt").await, b"fov:a2");

    a.write(on_a, "options.txt", b"fov:a3").await;
    b.write(copy, "options.txt", b"fov:b3").await;
    a.sync.sync_cluster(&a.state, &account, on_a).await.unwrap();
    assert!(matches!(
        b.sync.sync_cluster(&b.state, &account, copy).await.unwrap(),
        SyncOutcome::Conflict { .. }
    ));
    assert!(matches!(
        b.sync
            .resolve(&b.state, &account, copy, Resolution::KeepLocal)
            .await
            .unwrap(),
        SyncOutcome::Pushed { .. }
    ));
    assert!(matches!(
        a.sync.sync_cluster(&a.state, &account, on_a).await.unwrap(),
        SyncOutcome::Pulled { .. }
    ));
    assert_eq!(a.read(on_a, "options.txt").await, b"fov:b3");

    a.sync.disable(&account, b_copy_on_a).await.unwrap();
    assert_eq!(a.status(&account, b_copy_on_a).await, None);
    let report = b.sync.sync_all(&b.state, &account).await.unwrap();
    assert_eq!(report.removed, vec![on_b]);
    assert!(
        b.state.clusters.get(on_b).await.is_ok(),
        "the local copy stays"
    );
    assert_eq!(b.status(&account, on_b).await, None);

    a.sync.cluster_deleted(Some(&account), on_a).await.unwrap();
    let report = b.sync.sync_all(&b.state, &account).await.unwrap();
    assert_eq!(report.removed, vec![copy]);
    assert!(b.sync.statuses(&account).await.unwrap().is_empty());
}

#[test]
fn left_out_settings_stay_on_each_machine() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(left_out_settings());
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn left_out_settings() {
    let scratch = polyio::testing::ScratchDir::new("sync-left-out");
    let cloud = scratch.path().join("cloud");
    let a = Machine::new(scratch.path(), &cloud, "machine-a").await;
    let b = Machine::new(scratch.path(), &cloud, "machine-b").await;
    let account = SyncAccount {
        id: Uuid::new_v4(),
        name: "Steve".into(),
        token: None,
    };

    let on_a = create_instance(&a, "Left Out").await;
    a.write(on_a, "options.txt", b"fov:a").await;
    a.write(on_a, "config/shared.json", b"{\"v\":1}").await;
    a.sync.enable(&a.state, &account, on_a).await.unwrap();
    let report = b.sync.sync_all(&b.state, &account).await.unwrap();
    let on_b = report.created[0];
    assert_eq!(b.read(on_b, "options.txt").await, b"fov:a");

    let left_out: std::collections::BTreeSet<String> =
        ["options.txt".to_string()].into_iter().collect();
    assert_eq!(
        a.sync
            .set_left_out(&a.state, &account, on_a, left_out.clone())
            .await
            .unwrap(),
        SyncOutcome::Pushed { revision: 2 }
    );
    let report = b.sync.sync_all(&b.state, &account).await.unwrap();
    assert_eq!(report.pulled, vec![on_b]);
    assert_eq!(
        b.sync.left_out(on_b).await.unwrap(),
        left_out,
        "the choice follows the instance to other computers"
    );

    b.write(on_b, "options.txt", b"fov:b").await;
    assert_eq!(
        b.sync.sync_cluster(&b.state, &account, on_b).await.unwrap(),
        SyncOutcome::Unchanged
    );
    a.write(on_a, "options.txt", b"fov:a2").await;
    a.write(on_a, "config/shared.json", b"{\"v\":2}").await;
    assert_eq!(
        a.sync.sync_cluster(&a.state, &account, on_a).await.unwrap(),
        SyncOutcome::Pushed { revision: 3 }
    );
    let report = b.sync.sync_all(&b.state, &account).await.unwrap();
    assert_eq!(report.pulled, vec![on_b]);
    assert_eq!(b.read(on_b, "config/shared.json").await, b"{\"v\":2}");
    assert_eq!(b.read(on_b, "options.txt").await, b"fov:b");
    assert_eq!(a.read(on_a, "options.txt").await, b"fov:a2");
}
