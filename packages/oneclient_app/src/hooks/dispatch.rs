use oneclient_core::notification::{NotificationLevel, UserChoice};
use oneclient_core::settings::{GameSettingsProfile, LauncherSettings, ProfileUpdate};
use oneclient_db::models::ClusterId;

use crate::bridge::{BridgeCommand, OneClientBridge};
use crate::components::IconType;
use crate::notifications::NotificationSpec;

#[derive(Clone)]
pub struct BridgeDispatch {
    bridge: OneClientBridge,
}

impl BridgeDispatch {
    pub(crate) fn new(bridge: OneClientBridge) -> Self {
        Self { bridge }
    }

    pub fn send(&self, command: BridgeCommand) {
        self.bridge.send(command);
    }

    pub fn reload_settings(&self) {
        self.send(BridgeCommand::ReloadSettings);
    }

    pub fn save_settings(&self) {
        self.send(BridgeCommand::SaveSettings);
    }

    pub fn set_settings(&self, settings: LauncherSettings) {
        self.send(BridgeCommand::SetSettings { settings });
    }

    pub fn record_seen_version(&self, version: impl Into<String>) {
        self.send(BridgeCommand::RecordSeenVersion {
            version: version.into(),
        });
    }

    pub fn mark_onboarding_seen(&self) {
        self.send(BridgeCommand::MarkOnboardingSeen);
    }

    pub fn import_launcher(
        &self,
        source: oneclient_core::MigrationSource,
        folder_name: impl Into<String>,
        target: oneclient_core::ImportTarget,
    ) {
        self.send(BridgeCommand::ImportLauncher {
            source,
            folder_name: folder_name.into(),
            target,
        });
    }

    pub fn save_global_profile(&self, profile: GameSettingsProfile) {
        self.send(BridgeCommand::SaveGlobalProfile { profile });
    }

    pub fn update_global_profile(&self, update: ProfileUpdate) {
        self.send(BridgeCommand::UpdateGlobalProfile { update });
    }

    pub fn create_settings_profile(&self, name: impl Into<String>) {
        self.send(BridgeCommand::CreateSettingsProfile {
            name: name.into(),
        });
    }

    pub fn create_profile_from_global(
        &self,
        name: impl Into<String>,
        mem_max: Option<u32>,
        force_fullscreen: Option<bool>,
    ) {
        self.send(BridgeCommand::CreateProfileFromGlobal {
            name: name.into(),
            mem_max,
            force_fullscreen,
        });
    }

    pub fn upsert_named_profile(&self, profile: GameSettingsProfile) {
        self.send(BridgeCommand::UpsertNamedProfile { profile });
    }

    pub fn update_named_profile(&self, name: impl Into<String>, update: ProfileUpdate) {
        self.send(BridgeCommand::UpdateNamedProfile {
            name: name.into(),
            update,
        });
    }

    pub fn delete_named_profile(&self, name: impl Into<String>) {
        self.send(BridgeCommand::DeleteNamedProfile {
            name: name.into(),
        });
    }

    pub fn update_cluster_profile(
        &self,
        cluster_id: ClusterId,
        update: ProfileUpdate,
    ) {
        self.send(BridgeCommand::UpdateClusterProfile {
            cluster_id,
            update,
        });
    }

    pub fn create_and_assign_cluster_profile(
        &self,
        cluster_id: ClusterId,
        profile_name: impl Into<String>,
    ) {
        self.send(BridgeCommand::CreateAndAssignClusterProfile {
            cluster_id,
            profile_name: profile_name.into(),
        });
    }

    pub fn set_cluster_loader_version(
        &self,
        cluster_id: ClusterId,
        version: impl Into<String>,
    ) {
        self.send(BridgeCommand::SetClusterLoaderVersion {
            cluster_id,
            version: version.into(),
        });
    }

    pub fn install_java_runtime(&self, vendor: oneclient_core::java::JavaVendor, major: u32) {
        self.send(BridgeCommand::InstallJavaRuntime { vendor, major });
    }

    pub fn add_custom_java_runtime(&self, path: std::path::PathBuf) {
        self.send(BridgeCommand::AddCustomJavaRuntime { path });
    }

    pub fn remove_java_runtime(&self, path: impl Into<String>) {
        self.send(BridgeCommand::RemoveJavaRuntime { path: path.into() });
    }

    pub fn toggle_notification_center(&self) {
        self.send(BridgeCommand::ToggleNotificationCenter);
    }

    pub fn close_notification_center(&self) {
        self.send(BridgeCommand::CloseNotificationCenter);
    }

    pub fn clear_notification_inbox(&self) {
        self.send(BridgeCommand::ClearNotificationInbox);
    }

    pub fn dismiss_toast(&self, entry_id: u64) {
        self.send(BridgeCommand::DismissToast(entry_id));
    }

    pub fn bump_toast(&self, entry_id: u64) {
        self.send(BridgeCommand::BumpToast(entry_id));
    }

    pub fn mark_notification_read(&self, entry_id: u64) {
        self.send(BridgeCommand::MarkNotificationRead(entry_id));
    }

    pub fn dismiss_notification(&self, entry_id: u64) {
        self.send(BridgeCommand::DismissNotification(entry_id));
    }

    pub fn answer_prompt(&self, choice: UserChoice) {
        self.send(BridgeCommand::AnswerPrompt(choice));
    }

    pub fn notify(&self, title: impl Into<String>) -> NotificationBuilder {
        NotificationBuilder {
            dispatch: self.clone(),
            spec: NotificationSpec {
                title: title.into(),
                body: String::new(),
                level: NotificationLevel::Info,
                icon: None,
                progress: None,
                actions: Vec::new(),
            },
        }
    }

    pub fn send_test_progress(&self, current: u64, total: u64) {
        self.send(BridgeCommand::SendTestProgress { current, total });
    }

    pub fn launch_cluster(&self, cluster_id: ClusterId) {
        self.send(BridgeCommand::LaunchCluster { cluster_id });
    }

    pub fn kill_cluster(&self, cluster_id: ClusterId) {
        self.send(BridgeCommand::KillCluster { cluster_id });
    }

    pub fn dismiss_game_error(&self) {
        self.send(BridgeCommand::DismissGameError);
    }

    pub fn import_local_file(
        &self,
        cluster_id: ClusterId,
        content_type: oneclient_core::packages::ContentType,
        path: std::path::PathBuf,
    ) {
        self.send(BridgeCommand::ImportLocalFile {
            cluster_id,
            content_type,
            path,
        });
    }

    pub fn install_package(
        &self,
        cluster_id: ClusterId,
        provider: oneclient_core::packages::ProviderId,
        project_id: impl Into<String>,
        version_id: impl Into<String>,
    ) {
        self.send(BridgeCommand::InstallPackage {
            cluster_id,
            provider,
            project_id: project_id.into(),
            version_id: version_id.into(),
        });
    }

    pub fn install_bundle(
        &self,
        cluster_id: ClusterId,
        bundle_name: impl Into<String>,
        skip_compatibility: bool,
    ) {
        self.send(BridgeCommand::InstallBundle {
            cluster_id,
            bundle_name: bundle_name.into(),
            skip_compatibility,
        });
    }

    pub fn apply_bundle_updates(&self, cluster_id: ClusterId) {
        self.send(BridgeCommand::ApplyBundleUpdates { cluster_id });
    }

    pub fn sync_bundles(&self) {
        self.send(BridgeCommand::SyncBundles);
    }
}

#[must_use = "the notification is not dispatched until `.send()` is called"]
pub struct NotificationBuilder {
    dispatch: BridgeDispatch,
    spec: NotificationSpec,
}

impl NotificationBuilder {
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.spec.body = body.into();
        self
    }

    pub fn level(mut self, level: NotificationLevel) -> Self {
        self.spec.level = level;
        self
    }

    pub fn info(self) -> Self {
        self.level(NotificationLevel::Info)
    }

    pub fn error(self) -> Self {
        self.level(NotificationLevel::Error)
    }

    pub fn icon(mut self, icon: IconType) -> Self {
        self.spec.icon = Some(icon);
        self
    }

    pub fn progress(mut self, current: u64, total: u64) -> Self {
        self.spec.progress = Some((current, total));
        self
    }

    pub fn action(mut self, label: impl Into<String>) -> Self {
        self.spec.actions.push(label.into());
        self
    }

    pub fn actions(mut self, actions: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.spec.actions = actions.into_iter().map(Into::into).collect();
        self
    }

    pub fn send(self) {
        self.dispatch.send(BridgeCommand::SendNotification { spec: self.spec });
    }
}
