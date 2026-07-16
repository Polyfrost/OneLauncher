use std::path::PathBuf;

use oneclient_core::notification::UserChoice;
use oneclient_core::packages::{ContentType, ProviderId};
use oneclient_core::settings::{GameSettingsProfile, LauncherSettings, ProfileUpdate};
use oneclient_db::models::ClusterId;

use crate::notifications::{ClusterUpdateSummary, NotificationSpec};

#[derive(Debug)]
pub enum BridgeCommand {
    ReloadSettings,
    SaveSettings,
    SetSettings {
        settings: LauncherSettings,
    },
    RecordSeenVersion {
        version: String,
    },
    SetSeenChangelogVersion {
        version: String,
    },
    MarkOnboardingSeen,
    AcceptTos {
        terms_version: u32,
        privacy_version: u32,
    },

    SaveGlobalProfile {
        profile: GameSettingsProfile,
    },
    UpdateGlobalProfile {
        update: ProfileUpdate,
    },

    CreateSettingsProfile {
        name: String,
    },
    CreateProfileFromGlobal {
        name: String,
        mem_max: Option<u32>,
        force_fullscreen: Option<bool>,
    },
    UpsertNamedProfile {
        profile: GameSettingsProfile,
    },
    UpdateNamedProfile {
        name: String,
        update: ProfileUpdate,
    },
    DeleteNamedProfile {
        name: String,
    },

    UpdateClusterProfile {
        cluster_id: ClusterId,
        update: ProfileUpdate,
    },
    CreateAndAssignClusterProfile {
        cluster_id: ClusterId,
        profile_name: String,
    },
    SetClusterLoaderVersion {
        cluster_id: ClusterId,
        version: String,
    },

    InstallJavaRuntime {
        vendor: oneclient_core::java::JavaVendor,
        major: u32,
    },
    AddCustomJavaRuntime {
        path: PathBuf,
    },
    RemoveJavaRuntime {
        path: String,
    },

    ToggleNotificationCenter,
    CloseNotificationCenter,
    ToggleAccountSwitcher,
    CloseAccountSwitcher,
    ClearNotificationInbox,
    DismissToast(u64),
    BumpToast(u64),
    MarkNotificationRead(u64),
    DismissNotification(u64),
    AnswerPrompt(UserChoice),
    OpenClusterUpdate(ClusterUpdateSummary),
    CloseClusterUpdate,
    SendNotification {
        spec: NotificationSpec,
    },
    SendTestProgress {
        current: u64,
        total: u64,
    },

    LaunchCluster {
        cluster_id: ClusterId,
    },
    KillCluster {
        cluster_id: ClusterId,
    },
    DismissGameError,

    ImportLocalFile {
        cluster_id: ClusterId,
        content_type: ContentType,
        path: PathBuf,
    },
    InstallPackage {
        cluster_id: ClusterId,
        provider: ProviderId,
        project_id: String,
        version_id: String,
    },

    InstallBundle {
        cluster_id: ClusterId,
        bundle_name: String,
        skip_compatibility: bool,
    },
    ApplyBundleUpdates {
        cluster_id: ClusterId,
    },
    SyncBundles,

    ImportLauncher {
        source: oneclient_core::MigrationSource,
        folder_name: String,
        target: oneclient_core::ImportTarget,
    },
}
