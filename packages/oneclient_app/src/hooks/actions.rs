//! Everything the UI can ask for.
//!
//! A method per operation, each free to return a result. Core operations go
//! onto freya's UI-thread executor, since radio state is `!Send` and anything
//! that writes it must run there; pure UI toggles write it immediately.
//!
//! Always `spawn_forever`, never `spawn`. Freya's `spawn` binds a task to the
//! calling component's scope and cancels it on unmount; these are started from
//! event handlers, so a launch kicked off from a panel that then re-renders
//! would have its log tail and exit watcher killed underneath it. The work is
//! app-scoped, not component-scoped.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use freya::prelude::spawn_forever;
use freya::radio::RadioStation;
use oneclient_cluster::{ClusterStage, ClusterUpdate, GameSettingsProfile, ProfileUpdate};
use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_core::settings::LauncherSettings;
use oneclient_core::settings::store::{save_global_profile, save_settings_and_apply};
use oneclient_db::models::ClusterId;
use oneclient_events::{Answer, Level};
use tokio::sync::mpsc;

use crate::components::IconType;
use crate::launcher;
use crate::notifications::{
    ClusterUpdateSummary, NotificationAction, NotificationSpec, PendingPrompt,
};
use crate::state::{AppChannel, AppState, AsyncStatus};

/// How long the launch button stays disabled after a click, at minimum.
///
/// Over-disabling is the cheaper mistake: a button that is dead for two seconds
/// after a failed launch beats one that lets a second game start.
const LAUNCH_HOLD: Duration = Duration::from_secs(2);

/// Asks the event pump to do something it alone owns.
///
/// The pump holds the toast timers, so an action that adds or removes a toast
/// has to tell it to re-arm; and hover-pause is a property of the timers, not of
/// any state a component reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpSignal {
    /// The notification engine changed; re-arm toast timers.
    Reconcile,
    PauseToasts,
    ResumeToasts,
}

#[derive(Clone)]
pub struct Actions {
    station: RadioStation<AppState, AppChannel>,
    pump: mpsc::UnboundedSender<PumpSignal>,
}

impl Actions {
    #[must_use]
    pub fn new(
        station: RadioStation<AppState, AppChannel>,
        pump: mpsc::UnboundedSender<PumpSignal>,
    ) -> Self {
        Self { station, pump }
    }

    fn nudge(&self, signal: PumpSignal) {
        let _ = self.pump.send(signal);
    }

    /// Folds a change into the notification engine, then re-arms toast timers.
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

    // --- launcher settings -------------------------------------------------

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

    /// Applies a settings change in memory and publishes it, returning the
    /// snapshot to persist.
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

    /// Not persisted: "seen" versions are only used to decide whether to show
    /// the changelog this run, and the next real save carries them along.
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

    // --- settings profiles -------------------------------------------------

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
            // Best effort: the cluster is still usable if the stage reset fails,
            // it just will not re-download on next launch.
            let _ = state
                .clusters
                .set_stage(cluster_id, ClusterStage::NotReady)
                .await;
            super::invalidate_cluster_queries().await;
        });
    }

    // --- migration ---------------------------------------------------------

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

    // --- java --------------------------------------------------------------

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

    // --- notifications (pure UI) -------------------------------------------

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

    /// Hovering any toast pauses every toast, including ones that arrive while
    /// hovering, which is why this is a pump signal rather than a state flag.
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

    /// Debug-only: drives a fake progress notification through the real path.
    pub fn send_test_progress(&self, current: u64, total: u64) {
        let Ok(state) = launcher::state() else { return };
        let id = uuid::Uuid::from_u128(0x0CE0_0CE0_0CE0_0CE0_0CE0_0CE0_0CE0_0CE0);
        state
            .services
            .events
            .progress(id, "Downloading assets", current, total);
    }

    // --- game --------------------------------------------------------------

    /// Starts a launch, at most one per cluster.
    ///
    /// The pending flag is raised synchronously, before the first `await`, so
    /// the button is already disabled by the time a second click of a
    /// double-click could be dispatched; waiting for core to report `Checking`
    /// left a few hundred ms where every click spawned its own game. The claim
    /// itself is the guard, so a click that bypasses the disabled attribute
    /// (keyboard activation, replayed events) is dropped here too.
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
        spawn_forever(async move {
            let started = Instant::now();
            launch(cluster_id).await;

            // A launch that fails instantly would hand the button back inside
            // the same click burst, so hold it for the floor either way.
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

    // --- content -----------------------------------------------------------

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

            // The download already raised a progress notification; the outcome
            // replaces it in place rather than arriving as a second one.
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
                state
                    .services
                    .events
                    .signal(oneclient_events::Signal::ClustersChanged);
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

    /// Syncs the bundle catalog and brings every cluster up to date.
    ///
    /// `syncing_bundles` gates every launch button, so the flag must not be
    /// raised unless this can actually clear it, hence the readiness check before
    /// the write rather than inside the task.
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

            // One shared grouped session for the whole batch, so every cluster's
            // downloads surface as a single "update in progress" notification.
            // `detach` hands that entry over so it can be converted to the
            // finished "view changes" state in place rather than replaced.
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
}

/// The launch itself. Failures are reported as game events, so the caller only
/// has to know that the attempt is over.
async fn launch(cluster_id: ClusterId) {
    let Ok(state) = launcher::state() else { return };
    let events = state.services.events.clone();

    // Renews a lapsed token before launching.
    let account = match state.auth.default_account_for_launch().await {
        Ok(account) => account,
        Err(err) => {
            events.game_failed(cluster_id, format!("{err:#}"));
            return;
        }
    };

    match account {
        Some(account) => {
            if let Err(err) =
                oneclient_core::launch_cluster(&state, cluster_id, &account, true).await
            {
                events.game_failed(cluster_id, format!("{err:#}"));
            }
        }
        None => events.game_failed(
            cluster_id,
            "Add a Minecraft account before launching.".to_string(),
        ),
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
