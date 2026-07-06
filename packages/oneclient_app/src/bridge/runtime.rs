use oneclient_core::notification::{NotificationLevel, Notification, NotificationService};
use oneclient_core::packages::PackageStore;
use oneclient_core::settings::store::{
    self, create_profile_from_global, create_settings_profile, delete_named_profile,
    save_global_profile, save_settings, update_named_profile, upsert_named_profile,
};
use oneclient_core::{ClusterManager, LauncherState};
use tokio::sync::{mpsc, watch};

use crate::notifications::{
    InboxEntry, MESSAGE_TOAST_TTL, NotificationState, PendingPrompt, PendingPromptView,
};

use super::commands::BridgeCommand;
use super::snapshot::{AsyncStatus, BridgeSnapshot, LauncherInit};
use oneclient_core::notification::LaunchStage;

pub struct CoreBridgeRuntime {
    pub snapshots_tx: watch::Sender<BridgeSnapshot>,
    pub commands_rx: mpsc::UnboundedReceiver<BridgeCommand>,
    #[allow(dead_code)]
    pub commands_tx: mpsc::UnboundedSender<BridgeCommand>,
}

impl CoreBridgeRuntime {
    pub async fn run(mut self) {
        let mut snapshots = BridgeSnapshot::default();
        let mut notification_engine = NotificationState::default();
        let mut inbox: Vec<InboxEntry> = Vec::new();
        let mut notification_center_open = false;
        let mut pending_prompt: Option<PendingPrompt> = None;

        let mut armed_toasts: std::collections::HashMap<u64, tokio::time::Instant> =
            std::collections::HashMap::new();

        let mut game_flush_deadline: Option<tokio::time::Instant> = None;
        const GAME_LOG_FLUSH: std::time::Duration = std::time::Duration::from_millis(120);

        let (notifier_tx, mut notifier_rx) = mpsc::unbounded_channel();

        if let Err(err) = Self::initialize_launcher(&mut snapshots, notifier_tx).await {
            let message = err.to_string();
            snapshots.launcher.error = Some(message.clone());
            let (notif, _timers, _) = notification_engine.dispatch(
                &mut inbox,
                Notification::Message {
                    title: "Launcher failed to start".into(),
                    body: message,
                    level: NotificationLevel::Error,
                },
            );
            snapshots.notifications = notif;
            publish(&self.snapshots_tx, &snapshots);
            tracing::error!("launcher init failed: {err:#}");
        } else {
            sync_notifications(
                &mut snapshots,
                &notification_engine,
                &inbox,
                notification_center_open,
                &pending_prompt,
            );
            publish(&self.snapshots_tx, &snapshots);
        }

        loop {
            let next_deadline = armed_toasts
                .values()
                .min()
                .copied()
                .into_iter()
                .chain(game_flush_deadline)
                .min();
            let timer = async {
                match next_deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending::<()>().await,
                }
            };

            tokio::select! {
                _ = timer => {
                    let now = tokio::time::Instant::now();
                    let due: Vec<u64> = armed_toasts
                        .iter()
                        .filter(|(_, deadline)| **deadline <= now)
                        .map(|(id, _)| *id)
                        .collect();
                    let toasts_expired = !due.is_empty();
                    for id in due {
                        notification_engine.expire_toast(&inbox, id);
                        armed_toasts.remove(&id);
                    }

                    if game_flush_deadline.is_some_and(|d| d <= now) {
                        game_flush_deadline = None;
                    }

                    if toasts_expired {
                        sync_notifications(
                            &mut snapshots,
                            &notification_engine,
                            &inbox,
                            notification_center_open,
                            &pending_prompt,
                        );
                    }
                    publish(&self.snapshots_tx, &snapshots);
                    if toasts_expired {
                        reconcile_toasts(&snapshots, &inbox, &mut armed_toasts);
                    }
                }
                notification = notifier_rx.recv() => {
                    let Some(first) = notification else { break };

                    let mut touched_engine = false;
                    let mut immediate = false;
                    let mut saw_game_log = false;
                    let mut process = |notification,
                                       snapshots: &mut BridgeSnapshot,
                                       engine: &mut NotificationState,
                                       inbox: &mut Vec<InboxEntry>,
                                       pending: &mut Option<PendingPrompt>| {
                        match notification {
                            Notification::InvalidateClusters => {
                                bump_clusters_generation(snapshots);
                                immediate = true;
                            }
                            Notification::InvalidateJava => {
                                bump_java_generation(snapshots);
                                immediate = true;
                            }
                            Notification::SyncComplete => {
                                snapshots.launcher.fetching = false;
                                bump_clusters_generation(snapshots);
                                immediate = true;
                            }
                            Notification::GameStage { cluster_id, stage } => {
                                apply_game_stage(snapshots, cluster_id, stage);
                                immediate = true;
                            }
                            Notification::GameLog { cluster_id, line } => {
                                apply_game_log(snapshots, cluster_id, line);
                                saw_game_log = true;
                            }
                            Notification::GameFailed { cluster_id, message } => {
                                apply_game_failed(snapshots, cluster_id, message);
                                immediate = true;
                            }
                            other => {
                                let (notif, _timers, prompt) = engine.dispatch(inbox, other);
                                snapshots.notifications = notif;
                                if let Some(prompt) = prompt {
                                    *pending = Some(prompt);
                                }
                                touched_engine = true;
                            }
                        }
                    };

                    process(first, &mut snapshots, &mut notification_engine, &mut inbox, &mut pending_prompt);
                    let mut budget = 1024;
                    while budget > 0 {
                        match notifier_rx.try_recv() {
                            Ok(next) => {
                                process(next, &mut snapshots, &mut notification_engine, &mut inbox, &mut pending_prompt);
                                budget -= 1;
                            }
                            Err(_) => break,
                        }
                    }

                    if touched_engine {
                        sync_notifications(
                            &mut snapshots,
                            &notification_engine,
                            &inbox,
                            notification_center_open,
                            &pending_prompt,
                        );
                    }

                    if touched_engine || immediate {
                        publish(&self.snapshots_tx, &snapshots);
                        game_flush_deadline = None;
                        if touched_engine {
                            reconcile_toasts(&snapshots, &inbox, &mut armed_toasts);
                        }
                    } else if saw_game_log && game_flush_deadline.is_none() {
                        game_flush_deadline = Some(tokio::time::Instant::now() + GAME_LOG_FLUSH);
                    }
                }
                command = self.commands_rx.recv() => {
                    let Some(command) = command else { break };
                    match command {
                        BridgeCommand::BumpToast(entry_id) => {
                            if armed_toasts.contains_key(&entry_id) {
                                armed_toasts
                                    .insert(entry_id, tokio::time::Instant::now() + MESSAGE_TOAST_TTL);
                            }
                            continue;
                        }
                        other => {
                            if let Err(err) = Self::dispatch(
                                other,
                                &mut snapshots,
                                &mut notification_engine,
                                &mut inbox,
                                &mut notification_center_open,
                                &mut pending_prompt,
                            )
                            .await
                            {
                                tracing::error!("command failed: {err:#}");
                            }
                        }
                    }
                    sync_notifications(
                        &mut snapshots,
                        &notification_engine,
                        &inbox,
                        notification_center_open,
                        &pending_prompt,
                    );
                    publish(&self.snapshots_tx, &snapshots);
                    reconcile_toasts(&snapshots, &inbox, &mut armed_toasts);
                }
            }
        }
    }

    async fn initialize_launcher(
        snapshots: &mut BridgeSnapshot,
        notifier_tx: mpsc::UnboundedSender<Notification>,
    ) -> anyhow::Result<()> {
        let notifier = NotificationService::new(notifier_tx);
        LauncherState::initialize(notifier).await?;

        oneclient_core::status::start();

        let data_dir = oneclient_core::paths::launcher_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        snapshots.launcher = LauncherInit {
            ready: true,
            fetching: true,
            error: None,
            data_dir,
        };
        refresh_settings(snapshots)?;
        bump_profiles_generation(snapshots);

        let auto_update = LauncherState::get()?.settings.read().auto_update;
        crate::updater::spawn_update_check(auto_update);

        Ok(())
    }

    async fn dispatch(
        command: BridgeCommand,
        snapshots: &mut BridgeSnapshot,
        notification_engine: &mut NotificationState,
        inbox: &mut Vec<InboxEntry>,
        notification_center_open: &mut bool,
        pending_prompt: &mut Option<PendingPrompt>,
    ) -> anyhow::Result<()> {
        match command {
            BridgeCommand::ReloadSettings => {
                snapshots.settings.status = AsyncStatus::Loading;
                snapshots.settings.error = None;
                let state = LauncherState::get()?;
                let loaded = store::load_settings(Some(&state.services.notifier)).await;
                {
                    let mut lock = state.settings.write();
                    *lock = loaded;
                }
                refresh_settings(snapshots)?;
            }
            BridgeCommand::SaveSettings => {
                let state = LauncherState::get()?;
                snapshots.settings.saving = true;
                snapshots.settings.error = None;
                let settings = state.settings.read().clone();
                if let Err(err) = save_settings(&settings).await {
                    snapshots.settings.error = Some(err.to_string());
                }
                snapshots.settings.saving = false;
                snapshots.settings.status = AsyncStatus::Ready;
            }
            BridgeCommand::SetSettings { settings } => {
                mutate_settings(snapshots, |s| *s = settings)?;
                let state = LauncherState::get()?;
                let snapshot = state.settings.read().clone();
                if let Err(err) = save_settings(&snapshot).await {
                    snapshots.settings.error = Some(err.to_string());
                }
            }
            BridgeCommand::RecordSeenVersion { version } => {
                mutate_settings(snapshots, |settings| {
                    if !settings.seen_versions.iter().any(|v| v == &version) {
                        settings.seen_versions.push(version);
                    }
                })?;
            }
            BridgeCommand::MarkOnboardingSeen => {
                mutate_settings(snapshots, |settings| {
                    settings.seen_onboarding = true;
                })?;
                let state = LauncherState::get()?;
                let snapshot = state.settings.read().clone();
                if let Err(err) = save_settings(&snapshot).await {
                    snapshots.settings.error = Some(err.to_string());
                }
            }

            BridgeCommand::SaveGlobalProfile { profile } => {
                let state = LauncherState::get()?;
                save_global_profile(&state.settings, profile).await?;
                refresh_settings(snapshots)?;
                bump_profiles_generation(snapshots);
            }
            BridgeCommand::UpdateGlobalProfile { update } => {
                let state = LauncherState::get()?;
                let mut global = state.settings.read().global_game_settings.clone();
                update.apply(&mut global);
                save_global_profile(&state.settings, global).await?;
                refresh_settings(snapshots)?;
                bump_profiles_generation(snapshots);
            }

            BridgeCommand::CreateSettingsProfile { name } => {
                let state = LauncherState::get()?;
                let settings = state.settings.read().clone();
                create_settings_profile(&state.services.db, &settings, &name).await?;
                bump_profiles_generation(snapshots);
            }
            BridgeCommand::CreateProfileFromGlobal {
                name,
                mem_max,
                force_fullscreen,
            } => {
                let state = LauncherState::get()?;
                let settings = state.settings.read().clone();
                create_profile_from_global(
                    &state.services.db,
                    &settings,
                    &name,
                    mem_max,
                    force_fullscreen,
                )
                .await?;
                bump_profiles_generation(snapshots);
            }
            BridgeCommand::UpsertNamedProfile { profile } => {
                let state = LauncherState::get()?;
                upsert_named_profile(&state.services.db, &profile).await?;
                bump_profiles_generation(snapshots);
            }
            BridgeCommand::UpdateNamedProfile { name, update } => {
                let state = LauncherState::get()?;
                update_named_profile(&state.services.db, &name, update).await?;
                bump_profiles_generation(snapshots);
            }
            BridgeCommand::DeleteNamedProfile { name } => {
                let state = LauncherState::get()?;
                delete_named_profile(&state.services.db, &name).await?;
                bump_profiles_generation(snapshots);
            }

            BridgeCommand::UpdateClusterProfile { cluster_id, update } => {
                let state = LauncherState::get()?;
                ClusterManager::update_profile(&state, cluster_id, update).await?;
                bump_profiles_generation(snapshots);
            }
            BridgeCommand::CreateAndAssignClusterProfile {
                cluster_id,
                profile_name,
            } => {
                let state = LauncherState::get()?;
                ClusterManager::create_and_assign_profile(&state, cluster_id, &profile_name)
                    .await?;
                bump_profiles_generation(snapshots);
            }
            BridgeCommand::SetClusterLoaderVersion {
                cluster_id,
                version,
            } => {
                let state = LauncherState::get()?;
                ClusterManager::update(
                    &state,
                    cluster_id,
                    oneclient_core::ClusterUpdate::default().loader_version(version),
                )
                .await?;
                let _ = ClusterManager::set_stage(
                    &state,
                    cluster_id,
                    oneclient_core::ClusterStage::NotReady,
                )
                .await;
                bump_clusters_generation(snapshots);
            }

            BridgeCommand::InstallJavaRuntime { vendor, major } => {
                let state = LauncherState::get()?;
                tokio::spawn(async move {
                    let notifier = state.services.notifier.clone();
                    match oneclient_core::java::JavaManager::install_runtime_from(
                        &state.services,
                        &vendor,
                        major,
                    )
                    .await
                    {
                        Ok(runtime) => {
                            notifier.send_info(
                                "Java installed",
                                &format!("Java {} ({})", runtime.major, runtime.vendor),
                            );
                            notifier.invalidate_java();
                        }
                        Err(err) => {
                            notifier.send_error("Java install failed", &err.to_string());
                        }
                    }
                });
            }
            BridgeCommand::AddCustomJavaRuntime { path } => {
                let state = LauncherState::get()?;
                tokio::spawn(async move {
                    let notifier = state.services.notifier.clone();
                    match oneclient_core::java::JavaManager::add_custom_runtime(
                        &state.services.db,
                        path,
                    )
                    .await
                    {
                        Ok(runtime) => {
                            notifier.send_info(
                                "Java added",
                                &format!("Java {} ({})", runtime.major, runtime.vendor),
                            );
                            notifier.invalidate_java();
                        }
                        Err(err) => {
                            notifier.send_error("Failed to add Java", &err.to_string());
                        }
                    }
                });
            }
            BridgeCommand::RemoveJavaRuntime { path } => {
                let state = LauncherState::get()?;
                oneclient_core::java::JavaManager::remove_runtime(&state.services.db, &path)
                    .await?;
                bump_java_generation(snapshots);
            }

            BridgeCommand::ToggleNotificationCenter => {
                *notification_center_open =
                    notification_engine.toggle_center(inbox, *notification_center_open);
            }
            BridgeCommand::CloseNotificationCenter => {
                *notification_center_open = false;
            }
            BridgeCommand::ClearNotificationInbox => {
                inbox.clear();
                notification_engine.clear_inbox();
            }
            BridgeCommand::DismissToast(entry_id) => {
                notification_engine.dismiss_toast(inbox, entry_id);
            }
            BridgeCommand::BumpToast(_) => {}
            BridgeCommand::MarkNotificationRead(entry_id) => {
                notification_engine.mark_read(inbox, entry_id);
            }
            BridgeCommand::DismissNotification(entry_id) => {
                notification_engine.dismiss_notification(inbox, entry_id);
            }
            BridgeCommand::AnswerPrompt(choice) => {
                if let Some(prompt) = pending_prompt.take()
                    && let Some(reply_tx) = prompt.reply_tx
                {
                    let _ = reply_tx.send(choice);
                }
            }
            BridgeCommand::SendNotification { spec } => {
                notification_engine.push_custom(inbox, spec);
            }
            BridgeCommand::SendTestProgress { current, total } => {
                let state = LauncherState::get()?;
                let id = uuid::Uuid::from_u128(0x0CE0_0CE0_0CE0_0CE0_0CE0_0CE0_0CE0_0CE0);
                state
                    .services
                    .notifier
                    .send_progress(&id, "Downloading assets", current, total);
            }

            BridgeCommand::LaunchCluster { cluster_id } => {
                let state = LauncherState::get()?;
                tokio::spawn(async move {
                    let notifier = state.services.notifier.clone();
                    let account = match oneclient_core::auth::get_default_account().await {
                        Ok(Some(account)) => Some(account),
                        Ok(None) => oneclient_core::auth::list_accounts()
                            .await
                            .ok()
                            .and_then(|accounts| accounts.into_iter().next()),
                        Err(err) => {
                            notifier.game_failed(cluster_id, err.to_string());
                            return;
                        }
                    };
                    match account {
                        Some(account) => {
                            if let Err(err) =
                                oneclient_core::launch_cluster(&state, cluster_id, &account, true)
                                    .await
                            {
                                notifier.game_failed(cluster_id, format!("{err:#}"));
                            }
                        }
                        None => notifier.game_failed(
                            cluster_id,
                            "Add a Minecraft account before launching.".to_string(),
                        ),
                    }
                });
            }

            BridgeCommand::KillCluster { cluster_id } => {
                let state = LauncherState::get()?;
                if !state.games.kill(cluster_id) {
                    tracing::debug!(cluster_id, "kill requested but no tracked process");
                }
            }
            BridgeCommand::DismissGameError => {
                snapshots.game.error = None;
            }

            BridgeCommand::ImportLocalFile {
                cluster_id,
                content_type,
                path,
            } => {
                let state = LauncherState::get()?;
                match oneclient_core::packages::PackageStore::import_local_file(
                    &path,
                    content_type,
                    cluster_id,
                    &state.services,
                )
                .await
                {
                    Ok(row) => {
                        state
                            .services
                            .notifier
                            .send_info("Imported", &format!("Added {}", row.file_name));
                        bump_clusters_generation(snapshots);
                    }
                    Err(err) => state
                        .services
                        .notifier
                        .send_error("Import failed", &err.to_string()),
                }
            }
            BridgeCommand::InstallPackage {
                cluster_id,
                provider,
                project_id,
                version_id,
            } => {
                let state = LauncherState::get()?;
                match install_package(&state, provider, &project_id, &version_id, cluster_id).await {
                    Ok(()) => {
                        state
                            .services
                            .notifier
                            .send_info("Installed", "Package added to cluster");
                        bump_clusters_generation(snapshots);
                    }
                    Err(err) => state
                        .services
                        .notifier
                        .send_error("Install failed", &err.to_string()),
                }
            }

            BridgeCommand::InstallBundle {
                cluster_id,
                bundle_name,
                skip_compatibility,
            } => {
                let state = LauncherState::get()?;
                oneclient_core::install_bundle(
                    cluster_id,
                    &bundle_name,
                    skip_compatibility,
                    state.bundles.as_ref(),
                    &state.services,
                )
                .await?;
                bump_clusters_generation(snapshots);
            }
            BridgeCommand::ApplyBundleUpdates { cluster_id } => {
                let state = LauncherState::get()?;
                oneclient_core::apply_bundle_updates(
                    cluster_id,
                    state.bundles.as_ref(),
                    &state.services,
                )
                .await?;
                bump_clusters_generation(snapshots);
            }
            BridgeCommand::SyncBundles => {
                let state = LauncherState::get()?;
                state.bundles.sync(&state.services).await?;
                oneclient_core::clusters::ensure_from_bundles(&state).await?;
                oneclient_core::bundles::sync_all_cluster_bundles(
                    state.bundles.as_ref(),
                    &state.services,
                )
                .await;
                bump_clusters_generation(snapshots);
            }
        }

        Ok(())
    }
}

fn publish(tx: &watch::Sender<BridgeSnapshot>, snapshots: &BridgeSnapshot) {
    let _ = tx.send(snapshots.clone());
}

fn sync_notifications(
    snapshots: &mut BridgeSnapshot,
    engine: &NotificationState,
    inbox: &[InboxEntry],
    center_open: bool,
    pending: &Option<PendingPrompt>,
) {
    let pending_view = pending.as_ref().map(|prompt| PendingPromptView {
        title: prompt.title.clone(),
        question: prompt.question.clone(),
        kind: prompt.kind,
    });
    snapshots.notifications = engine.snapshot(inbox, center_open, pending_view);
}

fn reconcile_toasts(
    snapshots: &BridgeSnapshot,
    inbox: &[InboxEntry],
    armed: &mut std::collections::HashMap<u64, tokio::time::Instant>,
) {
    let want: std::collections::HashSet<u64> = snapshots
        .notifications
        .active_toast_entry_ids
        .iter()
        .filter(|id| inbox.iter().any(|e| e.id == **id && !e.is_loading))
        .copied()
        .collect();

    for id in &want {
        armed
            .entry(*id)
            .or_insert_with(|| tokio::time::Instant::now() + MESSAGE_TOAST_TTL);
    }

    armed.retain(|id, _| want.contains(id));
}

fn refresh_settings(snapshots: &mut BridgeSnapshot) -> anyhow::Result<()> {
    let state = LauncherState::get()?;
    snapshots.settings.settings = state.settings.read().clone();
    snapshots.settings.status = AsyncStatus::Ready;
    snapshots.settings.error = None;
    Ok(())
}

async fn install_package(
    state: &std::sync::Arc<LauncherState>,
    provider: oneclient_core::packages::ProviderId,
    project_id: &str,
    version_id: &str,
    cluster_id: i64,
) -> anyhow::Result<()> {
    let provider_impl = state.services.packages.get(provider)?;
    let project = provider_impl.get_project(project_id, &state.services).await?;
    let version = provider_impl
        .get_version(project_id, version_id, &state.services)
        .await?;
    
    PackageStore::install_to_cluster(
        provider,
        &project,
        &version,
        cluster_id,
        false,
        false,
        &state.services,
    )
    .await?;
    Ok(())
}

fn apply_game_stage(snapshots: &mut BridgeSnapshot, cluster_id: i64, stage: LaunchStage) {
    snapshots.game.stages.insert(cluster_id, stage);
    if stage == LaunchStage::Checking {
        snapshots.game.error = None;
        snapshots
            .game
            .logs
            .insert(cluster_id, std::sync::Arc::new(Vec::new()));
    }
}

fn apply_game_failed(snapshots: &mut BridgeSnapshot, cluster_id: i64, message: String) {
    snapshots.game.stages.insert(cluster_id, LaunchStage::Exited);
    snapshots.game.error = Some(message);
}

fn apply_game_log(snapshots: &mut BridgeSnapshot, cluster_id: i64, line: String) {
    let buffer = snapshots.game.logs.entry(cluster_id).or_default();
    std::sync::Arc::make_mut(buffer).push(std::sync::Arc::from(line));
}

fn bump_profiles_generation(snapshots: &mut BridgeSnapshot) {
    snapshots.profiles.generation = snapshots.profiles.generation.wrapping_add(1);
}

fn bump_clusters_generation(snapshots: &mut BridgeSnapshot) {
    snapshots.clusters.generation = snapshots.clusters.generation.wrapping_add(1);
}

fn bump_java_generation(snapshots: &mut BridgeSnapshot) {
    snapshots.java.generation = snapshots.java.generation.wrapping_add(1);
}

fn mutate_settings(
    snapshots: &mut BridgeSnapshot,
    mutate: impl FnOnce(&mut oneclient_core::settings::LauncherSettings),
) -> anyhow::Result<()> {
    let state = LauncherState::get()?;
    {
        let mut lock = state.settings.write();
        mutate(&mut lock);
        snapshots.settings.settings = lock.clone();
    }
    snapshots.settings.status = AsyncStatus::Ready;
    Ok(())
}
