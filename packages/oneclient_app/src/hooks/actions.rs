//! Core operations go onto freya's UI-thread executor radio state is `!Send`
//!
//! Always `spawn_forever` never `spawn` Freya's `spawn` cancels the task when
//! the calling component unmounts this work is app-scoped not component-scoped

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use freya::prelude::spawn_forever;
use freya::radio::RadioStation;
use oneclient_cluster::{
    ClusterStage, ClusterUpdate, GameSettingsProfile, PackageUpdateMode, ProfileUpdate,
};
use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_core::settings::LauncherSettings;
use oneclient_core::settings::store::{save_global_profile, save_settings_and_apply};
use oneclient_db::models::ClusterId;
use oneclient_events::{Answer, Level};
use tokio::sync::mpsc;

use crate::components::IconType;
use crate::launcher;
use crate::notifications::{
    ClusterUpdateSummary, NotificationAction, NotificationSpec, PackageUpdateGroup, PendingPrompt,
};
use crate::state::{AppChannel, AppState, AsyncStatus};

/// Over-disabling is the cheaper mistake a dead button beats a second game
const LAUNCH_HOLD: Duration = Duration::from_secs(2);

/// Mirrors `oneclient_net`'s reachability probe not reqwest's per-request
/// timeouts which are minutes long This sits between Play and the game
const UPDATE_CHECK_BUDGET: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq)]
enum LaunchUpdatePlan {
    Nothing,
    Apply(Vec<oneclient_core::BrowserPackageUpdate>),
    Prompt(Vec<oneclient_core::BrowserPackageUpdate>),
}

/// Kept pure and separate from the work it describes so the three-way mode is
/// testable without a launcher database or provider
///
/// `pending` has already had declined versions filtered out by
/// `BrowserUpdateCheck::pending`
fn plan_launch_updates(
    mode: PackageUpdateMode,
    pending: Vec<oneclient_core::BrowserPackageUpdate>,
) -> LaunchUpdatePlan {
    if pending.is_empty() {
        return LaunchUpdatePlan::Nothing;
    }

    match mode {
        // The cache was already written skipping only affects what is shown
        PackageUpdateMode::Skip => LaunchUpdatePlan::Nothing,
        PackageUpdateMode::Automatic => LaunchUpdatePlan::Apply(pending),
        PackageUpdateMode::Prompt => LaunchUpdatePlan::Prompt(pending),
    }
}

/// The pump alone owns the toast timers so adding or removing a toast has to
/// tell it to re-arm hover-pause is a timer property not component state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpSignal {
    Reconcile,
    PauseToasts,
    ResumeToasts,
}

#[derive(Clone)]
pub struct Actions {
    station: RadioStation<AppState, AppChannel>,
    pump: mpsc::UnboundedSender<PumpSignal>,
    events: oneclient_events::EventBus,
}

impl Actions {
    #[must_use]
    pub fn station(&self) -> RadioStation<AppState, AppChannel> {
        self.station
    }

    /// A channel handle, so a clone still feeds the pump set up at launch
    #[must_use]
    pub fn events(&self) -> oneclient_events::EventBus {
        self.events.clone()
    }

    #[must_use]
    pub fn new(
        station: RadioStation<AppState, AppChannel>,
        pump: mpsc::UnboundedSender<PumpSignal>,
        events: oneclient_events::EventBus,
    ) -> Self {
        Self {
            station,
            pump,
            events,
        }
    }

    fn nudge(&self, signal: PumpSignal) {
        let _ = self.pump.send(signal);
    }

    fn with_engine(&self, mutate: impl FnOnce(&mut AppState)) {
        {
            let mut guard = self.station.clone().write_channel(AppChannel::Notifications);
            mutate(&mut guard);
        }
        self.nudge(PumpSignal::Reconcile);
    }

    fn set_settings_error(&self, error: Option<String>) {
        self.station
            .clone()
            .write_channel(AppChannel::Settings)
            .settings
            .error = error;
    }

    pub fn reload_settings(&self) {
        let station = self.station;
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            {
                let mut guard = station.clone().write_channel(AppChannel::Settings);
                guard.settings.status = AsyncStatus::Loading;
                guard.settings.error = None;
            }

            let loaded = oneclient_core::settings::store::load_settings(Some(
                &state.services.events,
            ))
            .await;
            let discord_enabled = loaded.discord_enabled;
            *state.settings.write() = loaded.clone();
            state.discord.set_enabled(discord_enabled);

            let mut guard = station.clone().write_channel(AppChannel::Settings);
            guard.settings.settings = loaded;
            guard.settings.status = AsyncStatus::Ready;
            guard.settings.error = None;
        });
    }

    pub fn save_settings(&self) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            {
                let mut guard = actions.station.clone().write_channel(AppChannel::Settings);
                guard.settings.saving = true;
                guard.settings.error = None;
            }

            let settings = state.settings.read().clone();
            let result = save_settings_and_apply(&state.services, &settings).await;

            let mut guard = actions.station.clone().write_channel(AppChannel::Settings);
            guard.settings.error = result.err().map(|err| err.to_string());
            guard.settings.saving = false;
            guard.settings.status = AsyncStatus::Ready;
        });
    }

    fn mutate_settings(&self, mutate: impl FnOnce(&mut LauncherSettings)) -> Option<LauncherSettings> {
        let state = launcher::state().ok()?;
        let updated = {
            let mut lock = state.settings.write();
            mutate(&mut lock);
            lock.clone()
        };

        let mut guard = self.station.clone().write_channel(AppChannel::Settings);
        guard.settings.settings = updated.clone();
        guard.settings.status = AsyncStatus::Ready;
        Some(updated)
    }

    fn persist(&self, settings: LauncherSettings) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            if let Err(err) = save_settings_and_apply(&state.services, &settings).await {
                actions.set_settings_error(Some(err.to_string()));
            }
        });
    }

    pub fn set_settings(&self, settings: LauncherSettings) {
        let Some(updated) = self.mutate_settings(|s| *s = settings) else {
            return;
        };
        if let Ok(state) = launcher::state() {
            state.discord.set_enabled(updated.discord_enabled);
        }
        self.persist(updated);
    }

    /// Not persisted the next real save carries the seen versions along
    pub fn record_seen_version(&self, version: impl Into<String>) {
        let version = version.into();
        self.mutate_settings(|settings| {
            if !settings.seen_versions.iter().any(|v| v == &version) {
                settings.seen_versions.push(version);
            }
        });
    }

    pub fn set_seen_changelog_version(&self, version: impl Into<String>) {
        let version = version.into();
        let mut changed = false;
        let updated = self.mutate_settings(|settings| {
            if settings.seen_changelog_version.as_deref() != Some(version.as_str()) {
                settings.seen_changelog_version = Some(version);
                changed = true;
            }
        });

        if let (true, Some(updated)) = (changed, updated) {
            self.persist(updated);
        }
    }

    pub fn mark_onboarding_seen(&self) {
        if let Some(updated) = self.mutate_settings(|settings| settings.seen_onboarding = true) {
            self.persist(updated);
        }
    }

    pub fn accept_tos(&self, terms_version: u32, privacy_version: u32) {
        if let Some(updated) = self.mutate_settings(|settings| {
            settings.accepted_tos_version = terms_version;
            settings.accepted_privacy_version = privacy_version;
        }) {
            self.persist(updated);
        }
    }

    pub fn save_global_profile(&self, profile: GameSettingsProfile) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            if let Err(err) = save_global_profile(&state.settings, profile).await {
                tracing::error!("failed to save the global profile: {err:#}");
                return;
            }
            actions.mutate_settings(|_| {});
            super::invalidate_profile_queries().await;
        });
    }

    pub fn update_global_profile(&self, update: ProfileUpdate) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let mut global = state.settings.read().global_game_settings.clone();
            update.apply(&mut global);
            if let Err(err) = save_global_profile(&state.settings, global).await {
                tracing::error!("failed to update the global profile: {err:#}");
                return;
            }
            actions.mutate_settings(|_| {});
            super::invalidate_profile_queries().await;
        });
    }

    pub fn create_settings_profile(&self, name: impl Into<String>) {
        let name = name.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let global = state.settings.read().global_game_settings.clone();
            match oneclient_cluster::profiles::create_settings_profile(
                &state.services.db,
                &global,
                &name,
            )
            .await
            {
                Ok(_) => super::invalidate_profile_queries().await,
                Err(err) => tracing::error!("failed to create settings profile: {err:#}"),
            }
        });
    }

    pub fn create_profile_from_global(
        &self,
        name: impl Into<String>,
        mem_max: Option<u32>,
        force_fullscreen: Option<bool>,
    ) {
        let name = name.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let global = state.settings.read().global_game_settings.clone();
            match oneclient_cluster::profiles::create_profile_from_global(
                &state.services.db,
                &global,
                &name,
                mem_max,
                force_fullscreen,
            )
            .await
            {
                Ok(_) => super::invalidate_profile_queries().await,
                Err(err) => tracing::error!("failed to create profile: {err:#}"),
            }
        });
    }

    pub fn upsert_named_profile(&self, profile: GameSettingsProfile) {
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            match oneclient_cluster::profiles::upsert_named_profile(&state.services.db, &profile)
                .await
            {
                Ok(_) => super::invalidate_profile_queries().await,
                Err(err) => tracing::error!("failed to save profile: {err:#}"),
            }
        });
    }

    pub fn update_named_profile(&self, name: impl Into<String>, update: ProfileUpdate) {
        let name = name.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            match oneclient_cluster::profiles::update_named_profile(
                &state.services.db,
                &name,
                update,
            )
            .await
            {
                Ok(_) => super::invalidate_profile_queries().await,
                Err(err) => tracing::error!("failed to update profile: {err:#}"),
            }
        });
    }

    pub fn delete_named_profile(&self, name: impl Into<String>) {
        let name = name.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            match oneclient_cluster::profiles::delete_named_profile(&state.services.db, &name).await
            {
                Ok(()) => super::invalidate_profile_queries().await,
                Err(err) => tracing::error!("failed to delete profile: {err:#}"),
            }
        });
    }

    pub fn update_cluster_profile(&self, cluster_id: ClusterId, update: ProfileUpdate) {
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            match state.clusters.update_profile(cluster_id, update).await {
                Ok(_) => super::invalidate_profile_queries().await,
                Err(err) => tracing::error!("failed to update cluster profile: {err:#}"),
            }
        });
    }

    pub fn create_and_assign_cluster_profile(
        &self,
        cluster_id: ClusterId,
        profile_name: impl Into<String>,
    ) {
        let profile_name = profile_name.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let global = state.settings.read().global_game_settings.clone();
            match state
                .clusters
                .create_and_assign_profile(&global, cluster_id, &profile_name)
                .await
            {
                Ok(_) => super::invalidate_profile_queries().await,
                Err(err) => tracing::error!("failed to assign cluster profile: {err:#}"),
            }
        });
    }

    pub fn set_cluster_loader_version(&self, cluster_id: ClusterId, version: impl Into<String>) {
        let version = version.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            if let Err(err) = state
                .clusters
                .update(cluster_id, ClusterUpdate::default().loader_version(version))
                .await
            {
                tracing::error!("failed to set loader version: {err:#}");
                return;
            }
            // Best effort on failure the cluster just will not re-download
            let _ = state
                .clusters
                .set_stage(cluster_id, ClusterStage::NotReady)
                .await;
            super::invalidate_cluster_queries().await;
        });
    }

    pub fn import_launcher(
        &self,
        source: oneclient_core::MigrationSource,
        folder_name: impl Into<String>,
        target: oneclient_core::ImportTarget,
    ) {
        let folder_name = folder_name.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let events = state.services.events.clone();
            match oneclient_core::import_migration_game_dir(&state, source, &folder_name, target)
                .await
            {
                Ok(()) => {
                    events
                        .notify("Import complete")
                        .body(format!("Copied your files from {}.", source.display_name()))
                        .send();
                    events.signal(oneclient_events::Signal::ClustersChanged);
                }
                Err(err) => events
                    .notify("Import failed")
                    .body(err.to_string())
                    .error()
                    .send(),
            }
        });
    }

    pub fn install_java_runtime(&self, vendor: oneclient_java::JavaVendor, major: u32) {
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let events = state.services.events.clone();
            match state.java.install_runtime_from(&vendor, major).await {
                Ok(_) => events.signal(oneclient_events::Signal::JavaChanged),
                Err(err) => events
                    .notify("Java install failed")
                    .body(err.to_string())
                    .error()
                    .send(),
            }
        });
    }

    pub fn add_custom_java_runtime(&self, path: PathBuf) {
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let events = state.services.events.clone();
            match state.java.add_custom_runtime(path).await {
                Ok(runtime) => {
                    events
                        .notify("Java added")
                        .body(format!("Java {} ({})", runtime.major, runtime.vendor))
                        .send();
                    events.signal(oneclient_events::Signal::JavaChanged);
                }
                Err(err) => events
                    .notify("Failed to add Java")
                    .body(err.to_string())
                    .error()
                    .send(),
            }
        });
    }

    pub fn remove_java_runtime(&self, path: impl Into<String>) {
        let path = path.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            match state.java.remove_runtime(&path).await {
                Ok(()) => super::invalidate_java_queries().await,
                Err(err) => tracing::error!("failed to remove java runtime: {err:#}"),
            }
        });
    }

    pub fn toggle_notification_center(&self) {
        self.with_engine(|state| {
            state.center_open = state
                .notifications
                .toggle_center(&mut state.inbox, state.center_open);
        });
    }

    pub fn toggle_account_switcher(&self) {
        let open = self.station.peek().account_switcher_open;
        self.station
            .clone()
            .write_channel(AppChannel::AccountSwitcher)
            .account_switcher_open = !open;
    }

    pub fn close_account_switcher(&self) {
        self.station
            .clone()
            .write_channel(AppChannel::AccountSwitcher)
            .account_switcher_open = false;
    }

    pub fn close_notification_center(&self) {
        self.with_engine(|state| state.center_open = false);
    }

    pub fn clear_notification_inbox(&self) {
        self.with_engine(|state| {
            state.inbox.clear();
            state.notifications.clear_inbox();
        });
    }

    pub fn dismiss_toast(&self, entry_id: u64) {
        self.with_engine(|state| state.notifications.dismiss_toast(&mut state.inbox, entry_id));
    }

    pub fn mark_notification_read(&self, entry_id: u64) {
        self.with_engine(|state| state.notifications.mark_read(&mut state.inbox, entry_id));
    }

    pub fn dismiss_notification(&self, entry_id: u64) {
        self.with_engine(|state| {
            state
                .notifications
                .dismiss_notification(&mut state.inbox, entry_id);
        });
    }

    pub fn open_cluster_update(&self, summaries: Vec<ClusterUpdateSummary>) {
        self.with_engine(|state| {
            state.notifications.open_cluster_update(summaries);
            state.center_open = false;
        });
    }

    pub fn close_cluster_update(&self) {
        self.with_engine(|state| state.notifications.close_cluster_update());
    }

    pub fn close_package_updates(&self) {
        self.with_engine(|state| state.notifications.close_package_updates());
    }

    /// Hovering any toast pauses every toast including ones arriving while
    /// hovering hence a pump signal rather than a state flag
    pub fn pause_toasts(&self) {
        self.nudge(PumpSignal::PauseToasts);
    }

    pub fn resume_toasts(&self) {
        self.nudge(PumpSignal::ResumeToasts);
    }

    pub fn answer_prompt(&self, answer: Answer) {
        self.reply_to_prompt(Some(answer));
    }

    pub fn dismiss_prompt(&self) {
        self.reply_to_prompt(None);
    }

    fn reply_to_prompt(&self, answer: Option<Answer>) {
        let taken = self
            .station
            .clone()
            .write_channel(AppChannel::Notifications)
            .prompt
            .take();

        if let Some(PendingPrompt {
            reply_tx: Some(reply_tx),
            ..
        }) = taken
        {
            let _ = reply_tx.send(answer);
        }
    }

    pub fn notify(&self, title: impl Into<String>) -> NotificationBuilder {
        NotificationBuilder {
            actions: self.clone(),
            spec: NotificationSpec {
                title: title.into(),
                body: String::new(),
                level: Level::Info,
                icon: None,
                progress: None,
                actions: Vec::new(),
            },
        }
    }

    fn push_notification(&self, spec: NotificationSpec) {
        self.with_engine(|state| {
            state.notifications.push_custom(&mut state.inbox, spec);
        });
    }

    pub fn send_test_progress(&self, current: u64, total: u64) {
        let Ok(state) = launcher::state() else { return };
        let id = uuid::Uuid::from_u128(0x0CE0_0CE0_0CE0_0CE0_0CE0_0CE0_0CE0_0CE0);
        state
            .services
            .events
            .progress(id, "Downloading assets", current, total);
    }

    /// The pending flag is raised synchronously before the first `await` the
    /// claim itself is the guard against a double-click spawning two games
    pub fn launch_cluster(&self, cluster_id: ClusterId) {
        let claimed = self
            .station
            .clone()
            .write_channel(AppChannel::Game)
            .game
            .begin_launch(cluster_id);
        if !claimed {
            return;
        }

        let station = self.station;
        let actions = self.clone();
        spawn_forever(async move {
            let started = Instant::now();
            launch(&actions, cluster_id).await;

            // An instant failure would hand the button back inside the same
            // click burst so hold it for the floor either way
            if let Some(remaining) = LAUNCH_HOLD.checked_sub(started.elapsed()) {
                tokio::time::sleep(remaining).await;
            }
            station
                .clone()
                .write_channel(AppChannel::Game)
                .game
                .finish_launch(cluster_id);
        });
    }

    pub fn kill_cluster(&self, cluster_id: ClusterId) {
        let Ok(state) = launcher::state() else { return };
        if !state.games.kill(cluster_id) {
            tracing::debug!(cluster_id, "kill requested but no tracked process");
        }
    }

    pub fn dismiss_game_error(&self) {
        self.station
            .clone()
            .write_channel(AppChannel::Game)
            .game
            .error = None;
    }

    pub fn import_local_file(&self, cluster_id: ClusterId, content_type: ContentType, path: PathBuf) {
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let events = state.services.events.clone();
            match oneclient_content::packages::PackageStore::import_local_file(
                &path,
                content_type,
                cluster_id,
                &state.services.content(),
            )
            .await
            {
                Ok(row) => {
                    events
                        .notify("Imported")
                        .body(format!("Added {}", row.file_name))
                        .send();
                    super::invalidate_cluster_queries().await;
                }
                Err(err) => events
                    .notify("Import failed")
                    .body(err.to_string())
                    .error()
                    .send(),
            }
        });
    }

    pub fn install_package(
        &self,
        cluster_id: ClusterId,
        provider: ProviderId,
        project_id: impl Into<String>,
        version_id: impl Into<String>,
    ) {
        let (project_id, version_id) = (project_id.into(), version_id.into());
        let actions = self.clone();

        self.station
            .clone()
            .write_channel(AppChannel::Installs)
            .installs
            .begin(cluster_id, provider, project_id.clone());

        spawn_forever(async move {
            let finish = |actions: &Actions| {
                actions
                    .station
                    .clone()
                    .write_channel(AppChannel::Installs)
                    .installs
                    .finish(cluster_id, provider, &project_id);
            };

            let Ok(state) = launcher::state() else {
                finish(&actions);
                return;
            };

            let install = crate::install::install_package(
                &state,
                provider,
                &project_id,
                &version_id,
                cluster_id,
            )
            .await;

            // Replaces the download's progress notification in place rather
            // than arriving as a second one
            let spec = match &install.result {
                Ok(name) => NotificationSpec {
                    title: "Installed".to_string(),
                    body: crate::install::install_body(
                        name,
                        &install.dependencies,
                        &install.missing_dependencies,
                    ),
                    level: Level::Info,
                    icon: Some(IconType::Download01),
                    progress: None,
                    actions: Vec::new(),
                },
                Err(err) => NotificationSpec {
                    title: "Install failed".to_string(),
                    body: err.to_string(),
                    level: Level::Error,
                    icon: None,
                    progress: None,
                    actions: Vec::new(),
                },
            };

            let session_id = install.session_id.unwrap_or_else(uuid::Uuid::nil);
            actions.with_engine(|app| {
                app.notifications
                    .finish_grouped_as_actions(&mut app.inbox, session_id, Some(spec));
            });

            if install.result.is_ok() {
                // Before the refresh below so the version list never renders
                // the moment where both copies read as active
                if let Err(err) = oneclient_content::bundles::reconcile_duplicate_activity(
                    cluster_id,
                    &state.services.content(),
                )
                .await
                {
                    tracing::warn!(%err, "failed to resolve duplicate package versions");
                }

                super::invalidate_cluster_content_queries().await;

                state
                    .services
                    .events
                    .signal(oneclient_events::Signal::ClustersChanged);
            }

            finish(&actions);
        });
    }

    /// Shares the installs channel with [`Self::install_package`] so every row's
    /// button reads as busy while one is being acted on
    ///
    /// Recorded as a bundle override when a bundle owns the artifact otherwise
    /// the next sync puts the version straight back
    pub fn remove_package_version(
        &self,
        cluster_id: ClusterId,
        provider: ProviderId,
        project_id: impl Into<String>,
        hash: impl Into<String>,
        display_name: impl Into<String>,
    ) {
        let (project_id, hash, display_name) =
            (project_id.into(), hash.into(), display_name.into());
        let actions = self.clone();

        self.station
            .clone()
            .write_channel(AppChannel::Installs)
            .installs
            .begin(cluster_id, provider, project_id.clone());

        spawn_forever(async move {
            let finish = |actions: &Actions| {
                actions
                    .station
                    .clone()
                    .write_channel(AppChannel::Installs)
                    .installs
                    .finish(cluster_id, provider, &project_id);
            };

            let Ok(state) = launcher::state() else {
                finish(&actions);
                return;
            };

            let events = state.services.events.clone();
            match oneclient_core::remove_artifact_from_cluster(
                cluster_id,
                &hash,
                true,
                &state.services.content(),
            )
            .await
            {
                Ok(()) => {
                    events.notify("Removed").body(display_name).send();

                    // The row's next state is read off the cluster's content
                    // so refresh that before the busy flag drops
                    super::invalidate_cluster_content_queries().await;
                    events.signal(oneclient_events::Signal::ClustersChanged);
                }
                Err(err) => events
                    .notify("Remove failed")
                    .body(err.to_string())
                    .error()
                    .send(),
            }

            finish(&actions);
        });
    }

    pub fn install_bundle(
        &self,
        cluster_id: ClusterId,
        bundle_name: impl Into<String>,
        skip_compatibility: bool,
    ) {
        let bundle_name = bundle_name.into();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let events = state.services.events.clone();
            match oneclient_core::install_bundle(
                cluster_id,
                &bundle_name,
                skip_compatibility,
                state.bundles.as_ref(),
                &state.services.content(),
            )
            .await
            {
                Ok(_) => {
                    events
                        .notify("Installed")
                        .body(format!("Added {bundle_name}"))
                        .send();
                    events.signal(oneclient_events::Signal::ClustersChanged);
                }
                Err(err) => events
                    .notify("Install failed")
                    .body(err.to_string())
                    .error()
                    .send(),
            }
        });
    }

    pub fn apply_bundle_updates(&self, cluster_id: ClusterId) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let result = match oneclient_core::apply_bundle_updates(
                cluster_id,
                state.bundles.as_ref(),
                &state.services.content(),
            )
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    state
                        .services
                        .events
                        .notify("Update failed")
                        .body(err.to_string())
                        .error()
                        .send();
                    return;
                }
            };

            super::invalidate_cluster_queries().await;
            if let Some(spec) =
                crate::install::cluster_update_notification(cluster_id, &result, &state.services)
                    .await
            {
                actions.push_notification(spec);
            }
        });
    }

    /// `syncing_bundles` gates every launch button so the readiness check must
    /// happen before the flag is raised not inside the task
    pub fn sync_bundles(&self) {
        let state = match launcher::state() {
            Ok(state) => state,
            Err(err) => {
                tracing::error!("bundle sync skipped, launcher not ready: {err:#}");
                return;
            }
        };

        let actions = self.clone();
        self.station
            .clone()
            .write_channel(AppChannel::Launcher)
            .launcher
            .syncing_bundles = true;

        spawn_forever(async move {

            if let Err(err) = state.bundles.sync(&state.services.content()).await {
                tracing::error!("bundle catalog sync failed: {err:#}");
            }
            if let Err(err) = oneclient_core::clusters::apply_remote_migrations(&state).await {
                tracing::error!("cluster migrations failed: {err:#}");
            }
            if let Err(err) = oneclient_core::clusters::ensure_from_bundles(&state).await {
                tracing::error!("bundle cluster provisioning failed: {err:#}");
            }

            // One grouped session for the batch so all downloads surface as a
            // single notification `detach` lets it be converted in place
            let session = oneclient_events::GroupedProgressSession::start(
                &state.services.events,
                "Updating mods",
            );
            let changed = oneclient_content::bundles::sync_all_cluster_bundles(
                state.bundles.as_ref(),
                &state.services.content(),
                Some(&session),
            )
            .await;
            let session_id = session.detach();

            actions
                .station
                .clone()
                .write_channel(AppChannel::Launcher)
                .launcher
                .syncing_bundles = false;
            super::invalidate_cluster_queries().await;

            let spec = crate::install::combined_cluster_update_spec(&changed, &state.services).await;
            actions.with_engine(|app| {
                app.notifications
                    .finish_grouped_as_actions(&mut app.inbox, session_id, spec);
            });
        });
    }

    /// The check runs and the cache is written whatever the mode says keeping
    /// the package manager's "Update available" markers honest
    ///
    /// Returns once the launch may proceed nothing is still downloading then
    /// Never fails the launch every error logs and lets the game start
    async fn resolve_package_updates_before_launch(
        &self,
        state: &Arc<oneclient_core::LauncherState>,
        cluster_id: ClusterId,
    ) {
        let content = state.services.content();

        // Bounded a hanging network would otherwise hold the launch for
        // reqwest's much longer timeouts
        let check = match tokio::time::timeout(
            UPDATE_CHECK_BUDGET,
            oneclient_core::refresh_browser_package_updates(cluster_id, &content),
        )
        .await
        {
            Ok(Ok(check)) => check,
            Ok(Err(err)) => {
                tracing::warn!(
                    cluster_id,
                    error = %err,
                    "browser package update check failed, launching anyway"
                );
                return;
            }
            Err(_elapsed) => {
                tracing::debug!(
                    cluster_id,
                    "browser package update check exceeded its launch budget"
                );
                return;
            }
        };

        let pending = check.pending();
        let mode = self.cluster_update_mode(state, cluster_id).await;

        match plan_launch_updates(mode, pending) {
            LaunchUpdatePlan::Nothing => {}
            LaunchUpdatePlan::Apply(updates) => {
                self.apply_updates_for_launch(state, &updates).await;
            }
            LaunchUpdatePlan::Prompt(updates) => {
                let Some(group) =
                    crate::install::package_update_group(cluster_id, &updates, &state.services)
                        .await
                else {
                    return;
                };
                self.prompt_package_updates(group).await;
            }
        }
    }

    /// Anything unreadable falls back to the default rather than blocking the
    /// launch on a settings lookup
    async fn cluster_update_mode(
        &self,
        state: &Arc<oneclient_core::LauncherState>,
        cluster_id: ClusterId,
    ) -> PackageUpdateMode {
        let global = state.settings.read().global_game_settings.clone();

        let Ok(cluster) = state.clusters.get(cluster_id).await else {
            return PackageUpdateMode::default();
        };

        state
            .clusters
            .resolve_settings(&global, &cluster)
            .await
            .ok()
            .and_then(|profile| profile.browser_update_mode)
            .unwrap_or_default()
    }

    /// One grouped progress notification so the pre-launch pause shows something
    async fn apply_updates_for_launch(
        &self,
        state: &Arc<oneclient_core::LauncherState>,
        updates: &[oneclient_core::BrowserPackageUpdate],
    ) {
        let content = state.services.content();
        let session = oneclient_events::GroupedProgressSession::start(
            &state.services.events,
            "Updating packages",
        );
        session.expect(
            oneclient_events::TaskCategory::Packages,
            updates.len() as u64,
            updates.len() as u64,
        );

        let mut applied = 0usize;
        for update in updates {
            let child = session.child(
                update.display_name.clone(),
                1,
                oneclient_events::TaskCategory::Packages,
            );
            match oneclient_core::apply_browser_package_update(update, Some(&child), &content).await
            {
                Ok(_) => applied += 1,
                // Not a reason to hold the game back the cache row survives
                // so the next launch offers it again
                Err(err) => tracing::warn!(
                    package = %update.display_name,
                    error = %err,
                    "automatic package update failed"
                ),
            }
            child.finish();
        }

        let session_id = session.detach();
        let spec = (applied > 0).then(|| NotificationSpec {
            title: "Packages updated".to_string(),
            body: format!(
                "{applied} package{} updated before launch",
                if applied == 1 { "" } else { "s" }
            ),
            level: Level::Info,
            icon: Some(IconType::DownloadCloud02),
            progress: None,
            actions: Vec::new(),
        });

        self.with_engine(|app| {
            app.notifications
                .finish_grouped_as_actions(&mut app.inbox, session_id, spec);
        });

        if applied > 0 {
            super::invalidate_cluster_queries().await;
        }
    }

    async fn prompt_package_updates(&self, group: PackageUpdateGroup) {
        let (done, wait) = tokio::sync::oneshot::channel();

        self.with_engine(move |state| {
            state.notifications.open_package_updates(vec![group], Some(done));
            state.center_open = false;
        });

        // A replaced or torn-down modal is equally a reason to stop waiting
        let _ = wait.await;
    }

    pub fn apply_package_update(&self, update: oneclient_core::BrowserPackageUpdate) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let events = state.services.events.clone();

            let session = oneclient_events::GroupedProgressSession::start(
                &events,
                format!("Updating {}", update.display_name),
            );
            let child = session.child(
                update.display_name.clone(),
                1,
                oneclient_events::TaskCategory::Packages,
            );

            let result = oneclient_core::apply_browser_package_update(
                &update,
                Some(&child),
                &state.services.content(),
            )
            .await;

            child.finish();
            let session_id = session.detach();

            let spec = match &result {
                Ok(_) => NotificationSpec {
                    title: "Updated".to_string(),
                    body: format!(
                        "{} is now on {}",
                        update.display_name, update.latest_version_name
                    ),
                    level: Level::Info,
                    icon: Some(IconType::DownloadCloud02),
                    progress: None,
                    actions: Vec::new(),
                },
                Err(err) => NotificationSpec {
                    title: "Update failed".to_string(),
                    body: err.to_string(),
                    level: Level::Error,
                    icon: None,
                    progress: None,
                    actions: Vec::new(),
                },
            };

            actions.with_engine(|app| {
                app.notifications
                    .finish_grouped_as_actions(&mut app.inbox, session_id, Some(spec));
            });

            // A failed update stays in the list so the user can retry
            if result.is_ok() {
                actions.with_engine(|app| {
                    app.notifications
                        .resolve_package_update(update.cluster_id, &update.hash);
                });
                super::invalidate_cluster_queries().await;
            }
        });
    }

    /// The package stays marked out of date only the modal stops asking and
    /// only for this version
    pub fn skip_package_update(&self, cluster_id: ClusterId, hash: impl Into<String>) {
        let hash = hash.into();
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            if let Err(err) = oneclient_core::skip_browser_package_update(
                cluster_id,
                &hash,
                &state.services.content(),
            )
            .await
            {
                tracing::warn!(error = %err, "failed to record a skipped package update");
            }

            actions.with_engine(|app| {
                app.notifications.resolve_package_update(cluster_id, &hash);
            });
            super::invalidate_cluster_queries().await;
        });
    }
}

/// Failures are reported as game events so the caller only has to know the
/// attempt is over Everything before `launch_cluster` is pre-launch work
async fn launch(actions: &Actions, cluster_id: ClusterId) {
    let Ok(state) = launcher::state() else { return };
    let events = state.services.events.clone();

    events.game_stage(cluster_id, oneclient_events::LaunchStage::Checking);

    let account = match state.auth.default_account_for_launch().await {
        Ok(account) => account,
        Err(err) => {
            events.game_failed(cluster_id, format!("{err:#}"));
            return;
        }
    };

    let Some(account) = account else {
        events.game_failed(
            cluster_id,
            "Add a Minecraft account before launching.".to_string(),
        );
        return;
    };

    // Before the game process never after Minecraft reads its mods once at
    // startup
    actions
        .resolve_package_updates_before_launch(&state, cluster_id)
        .await;

    if let Err(err) = oneclient_core::launch_cluster(&state, cluster_id, &account, true).await {
        // A missing file is the one failure the launcher can fix itself and a
        // path inside our metadata folder gives the user nothing to act on
        if err.indicates_missing_files() {
            repair_and_relaunch(&state, cluster_id, &account, err).await;
            return;
        }

        events.game_failed(cluster_id, format!("{err:#}"));
    }
}

/// Only ever one retry if the game still will not start after a full rehash and
/// repair the problem is not missing files
async fn repair_and_relaunch(
    state: &std::sync::Arc<oneclient_core::LauncherState>,
    cluster_id: ClusterId,
    account: &oneclient_auth::MinecraftAccount,
    original: oneclient_core::LauncherError,
) {
    let events = state.services.events.clone();

    tracing::warn!(
        cluster_id,
        "launch failed on a missing file; repairing: {original:#}"
    );
    events
        .notify("Repairing installation")
        .body("Some game files are missing. Checking and re-downloading them.")
        .send();

    let report = match oneclient_core::verify_cluster_files(state, cluster_id).await {
        Ok(report) => report,
        Err(repair_err) => {
            tracing::error!(cluster_id, "repair failed: {repair_err:#}");
            // Lead with the original failure the repair failure explains why
            // it was not fixed
            events.game_failed(
                cluster_id,
                format!("{original:#} (repair also failed: {repair_err:#})"),
            );
            return;
        }
    };

    events
        .notify("Repair complete")
        .body(report.summary())
        .send();

    if let Err(err) = oneclient_core::launch_cluster(state, cluster_id, account, true).await {
        tracing::error!(cluster_id, "launch failed again after repair: {err:#}");
        events.game_failed(cluster_id, format!("{err:#}"));
    }
}

#[must_use = "the notification is not raised until `.send()` is called"]
pub struct NotificationBuilder {
    actions: Actions,
    spec: NotificationSpec,
}

impl NotificationBuilder {
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.spec.body = body.into();
        self
    }

    pub fn level(mut self, level: Level) -> Self {
        self.spec.level = level;
        self
    }

    pub fn info(self) -> Self {
        self.level(Level::Info)
    }

    pub fn error(self) -> Self {
        self.level(Level::Error)
    }

    pub fn icon(mut self, icon: IconType) -> Self {
        self.spec.icon = Some(icon);
        self
    }

    pub fn progress(mut self, current: u64, total: u64) -> Self {
        self.spec.progress = Some((current, total));
        self
    }

    pub fn action(mut self, action: NotificationAction) -> Self {
        self.spec.actions.push(action);
        self
    }

    pub fn actions(mut self, actions: impl IntoIterator<Item = NotificationAction>) -> Self {
        self.spec.actions = actions.into_iter().collect();
        self
    }

    pub fn send(self) {
        self.actions.push_notification(self.spec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneclient_common::domain::ProviderId;

    fn update(hash: &str) -> oneclient_core::BrowserPackageUpdate {
        oneclient_core::BrowserPackageUpdate {
            cluster_id: 1,
            hash: hash.into(),
            provider: ProviderId::Modrinth,
            project_id: "sodium".into(),
            installed_version_id: "v1".into(),
            installed_version_name: "1.0".into(),
            latest_version_id: "v2".into(),
            latest_version_name: "2.0".into(),
            display_name: "Sodium".into(),
            skipped: false,
        }
    }

    #[test]
    fn skip_mode_launches_without_asking_or_installing() {
        assert_eq!(
            plan_launch_updates(PackageUpdateMode::Skip, vec![update("a")]),
            LaunchUpdatePlan::Nothing,
        );
    }

    #[test]
    fn automatic_mode_installs_without_asking() {
        assert_eq!(
            plan_launch_updates(PackageUpdateMode::Automatic, vec![update("a")]),
            LaunchUpdatePlan::Apply(vec![update("a")]),
        );
    }

    #[test]
    fn prompt_mode_is_the_default_and_asks() {
        assert_eq!(
            plan_launch_updates(PackageUpdateMode::default(), vec![update("a")]),
            LaunchUpdatePlan::Prompt(vec![update("a")]),
        );
    }

    #[test]
    fn nothing_pending_never_opens_a_modal() {
        for mode in PackageUpdateMode::ALL.iter().copied() {
            assert_eq!(
                plan_launch_updates(mode, Vec::new()),
                LaunchUpdatePlan::Nothing,
                "{mode:?} must not hold a launch up over an empty list",
            );
        }
    }
}
