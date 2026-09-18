use std::collections::HashMap;

use freya::prelude::spawn_forever;
use std::collections::HashSet;

use oneclient_content::packages::release_migration::{
    ReleaseMigrationPackage, ReleaseMigrationPlan, add_to_waitlist,
    apply_release_migration_dependency, apply_release_migration_package, plan_release_migration,
    plan_release_migrations, process_waitlist,
};
use oneclient_core::ReleaseTarget;
use oneclient_core::clusters::{
    Cluster, OfferLookup, ReleaseMigrationOffer, manual_migration_offer, record_new_versions,
    release_migration_offer,
};
use oneclient_events::Level;

use super::Actions;
use crate::components::IconType;
use crate::launcher;
use crate::notifications::NotificationSpec;
use crate::state::{AppChannel, PromptOrigin, ReleaseMigrationPrompt, ReleasePlanState};

enum SourcePlan {
    Offer(ReleaseMigrationPlan),
    Nothing,
    Unreachable,
}

fn classify(plan: Result<ReleaseMigrationPlan, oneclient_content::ContentError>) -> SourcePlan {
    match plan {
        Ok(plan) if plan.unreachable => SourcePlan::Unreachable,
        Ok(plan) if plan.packages.is_empty() => SourcePlan::Nothing,
        Ok(plan) => SourcePlan::Offer(plan),
        Err(err) => {
            tracing::warn!(error = %err, "release migration plan failed");
            SourcePlan::Unreachable
        }
    }
}

enum SourcePlans {
    Ready {
        sources: Vec<Cluster>,
        plans: HashMap<i64, ReleasePlanState>,
    },
    Nothing,
    Unreachable,
}

async fn plan_sources(
    offer: &ReleaseMigrationOffer,
    bundles: &oneclient_content::bundles::BundlesManager,
    content: &oneclient_content::ContentCtx,
) -> SourcePlans {
    let ids: Vec<i64> = offer.sources.iter().map(|source| source.id).collect();
    let mut checked: HashMap<i64, SourcePlan> = plan_release_migrations(&ids, offer.target.id, bundles, content)
        .await
        .into_iter()
        .map(|(source_id, plan)| (source_id, classify(plan)))
        .collect();

    let mut sources = Vec::new();
    let mut plans = HashMap::new();

    for source in &offer.sources {
        match checked.remove(&source.id) {
            Some(SourcePlan::Offer(plan)) => {
                plans.insert(source.id, ReleasePlanState::Ready(plan));
                sources.push(source.clone());
            }
            Some(SourcePlan::Nothing) | None => {}
            Some(SourcePlan::Unreachable) => return SourcePlans::Unreachable,
        }
    }

    if sources.is_empty() {
        SourcePlans::Nothing
    } else {
        SourcePlans::Ready { sources, plans }
    }
}

impl Actions {
    fn write_release_migration(&self, edit: impl FnOnce(&mut Option<ReleaseMigrationPrompt>)) {
        let mut guard = self
            .station
            .clone()
            .write_channel(AppChannel::ReleaseMigration);
        edit(&mut guard.release_migration);
    }

    fn remove_pending_release_migrations(&self, keys: Vec<String>) {
        if keys.is_empty() {
            return;
        }
        self.edit_settings(move |settings| {
            settings
                .pending_release_migrations
                .retain(|key| !keys.contains(key));
        });
    }

    pub fn check_release_migration(&self) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };

            if let Err(err) = record_new_versions(&state).await {
                tracing::warn!(error = %err, "could not record new versions for migration");
            }
            actions.refresh_settings_from_core();

            let (pending, onboarded) = {
                let settings = state.settings.read();
                (settings.pending_release_migrations.clone(), settings.seen_onboarding)
            };
            if pending.is_empty() {
                return;
            }

            if !onboarded {
                actions.remove_pending_release_migrations(pending);
                return;
            }

            let content = state.services.content();
            let mut finished = Vec::new();

            for key in pending {
                let Some(release) = ReleaseTarget::from_key(&key) else {
                    finished.push(key);
                    continue;
                };

                let offer = match release_migration_offer(&state, &release).await {
                    Ok(OfferLookup::Offer(offer)) => offer,
                    Ok(OfferLookup::NoSources) => {
                        finished.push(key);
                        continue;
                    }
                    Ok(OfferLookup::MissingCluster) => continue,
                    Err(err) => {
                        tracing::warn!(error = %err, key, "release migration lookup failed");
                        continue;
                    }
                };

                let (sources, plans) = match plan_sources(&offer, state.bundles.as_ref(), &content).await {
                    SourcePlans::Ready { sources, plans } => (sources, plans),
                    SourcePlans::Nothing => {
                        finished.push(key);
                        continue;
                    }
                    SourcePlans::Unreachable => continue,
                };

                let java_major = oneclient_core::required_java_major(&state, offer.target.id)
                    .await
                    .ok()
                    .flatten();

                actions.remove_pending_release_migrations(finished);
                let selected = sources[0].id;
                actions.write_release_migration(move |prompt| {
                    *prompt = Some(ReleaseMigrationPrompt {
                        key,
                        target: offer.target,
                        java_major,
                        sources,
                        selected,
                        plans,
                        origin: PromptOrigin::NewRelease,
                    });
                });
                return;
            }

            actions.remove_pending_release_migrations(finished);
        });
    }

    pub fn select_release_migration_source(&self, source_id: i64) {
        let mut target_id = None;
        self.write_release_migration(|prompt| {
            let Some(prompt) = prompt.as_mut() else { return };
            prompt.selected = source_id;
            if !prompt.plans.contains_key(&source_id) {
                prompt.plans.insert(source_id, ReleasePlanState::Loading);
                target_id = Some(prompt.target.id);
            }
        });

        let Some(target_id) = target_id else { return };
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let result = plan_release_migration(
                source_id,
                target_id,
                state.bundles.as_ref(),
                &state.services.content(),
            )
            .await;
            let next = match result {
                Ok(plan) if !plan.unreachable => ReleasePlanState::Ready(plan),
                Ok(_) => ReleasePlanState::Failed,
                Err(err) => {
                    tracing::warn!(error = %err, source_id, "release migration plan failed");
                    ReleasePlanState::Failed
                }
            };
            actions.write_release_migration(move |prompt| {
                if let Some(prompt) = prompt.as_mut() {
                    prompt.plans.insert(source_id, next);
                }
            });
        });
    }

    pub fn simulate_release_migration(&self, target_cluster_id: i64) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };

            let target = match state.clusters.get(target_cluster_id).await {
                Ok(target) => target,
                Err(err) => {
                    actions
                        .notify("Release migration simulation failed")
                        .body(err.to_string())
                        .level(Level::Error)
                        .send();
                    return;
                }
            };

            let release = ReleaseTarget {
                mc_version: target.mc_version.clone(),
                loader: target.mc_loader,
            };

            let offer = match release_migration_offer(&state, &release).await {
                Ok(OfferLookup::Offer(offer)) => offer,
                Ok(OfferLookup::NoSources | OfferLookup::MissingCluster) => {
                    actions
                        .notify("Nothing to migrate")
                        .body(
                            "All found packages were migrated"
                        )
                        .level(Level::Error)
                        .send();
                    return;
                }
                Err(err) => {
                    actions
                        .notify("Release migration simulation failed")
                        .body(err.to_string())
                        .level(Level::Error)
                        .send();
                    return;
                }
            };

            let (sources, plans) = match plan_sources(&offer, state.bundles.as_ref(), &state.services.content()).await {
                SourcePlans::Ready { sources, plans } => (sources, plans),
                SourcePlans::Nothing => {
                    actions
                        .notify("Nothing to migrate")
                        .body(
                            "All found packages were migrated"
                        )
                        .level(Level::Error)
                        .send();
                    return;
                }
                SourcePlans::Unreachable => {
                    actions
                        .notify("Release migration simulation failed")
                        .body("Couldn't reach Modrinth or CurseForge.")
                        .level(Level::Error)
                        .send();
                    return;
                }
            };

            let java_major = oneclient_core::required_java_major(&state, offer.target.id)
                .await
                .ok()
                .flatten();

            let selected = sources[0].id;
            actions.write_release_migration(move |prompt| {
                *prompt = Some(ReleaseMigrationPrompt {
                    key: offer.release.key(),
                    target: offer.target,
                    java_major,
                    sources,
                    selected,
                    plans,
                    origin: PromptOrigin::Simulated,
                });
            });
        });
    }

    fn set_release_migration_checking(&self, cluster_id: i64, checking: bool) {
        let mut guard = self
            .station
            .clone()
            .write_channel(AppChannel::ReleaseMigration);
        if checking {
            guard.release_migration_checking.insert(cluster_id);
        } else {
            guard.release_migration_checking.remove(&cluster_id);
        }
    }

    pub fn open_manual_migration(&self, target_cluster_id: i64, source_cluster_id: i64) {
        self.set_release_migration_checking(target_cluster_id, true);
        let actions = self.clone();
        spawn_forever(async move {
            actions
                .run_manual_migration(target_cluster_id, source_cluster_id)
                .await;
            actions.set_release_migration_checking(target_cluster_id, false);
        });
    }

    async fn run_manual_migration(&self, target_cluster_id: i64, source_cluster_id: i64) {
        let Ok(state) = launcher::state() else { return };

        let source = state
            .clusters
            .get(source_cluster_id)
            .await
            .map(|cluster| cluster.name)
            .unwrap_or_else(|_| "that cluster".to_string());
        let nothing = |target: &Cluster| {
            self.notify("Nothing to migrate")
                .body(
                    "No packages to migrate"
                )
                .send();
        };

        let offer = match manual_migration_offer(&state, target_cluster_id, source_cluster_id).await {
            Ok(OfferLookup::Offer(offer)) => offer,
            Ok(OfferLookup::NoSources) => {
                if let Ok(target) = state.clusters.get(target_cluster_id).await {
                    nothing(&target);
                }
                return;
            }
            Ok(OfferLookup::MissingCluster) => return,
            Err(err) => {
                tracing::warn!(error = %err, target_cluster_id, "manual migration lookup failed");
                self.notify("Couldn't check for packages to migrate")
                    .body(err.to_string())
                    .level(Level::Error)
                    .send();
                return;
            }
        };

        let (sources, plans) = match plan_sources(&offer, state.bundles.as_ref(), &state.services.content()).await {
            SourcePlans::Ready { sources, plans } => (sources, plans),
            SourcePlans::Nothing => {
                nothing(&offer.target);
                return;
            }
            SourcePlans::Unreachable => {
                self.notify("Couldn't check for packages to migrate")
                    .body("Couldn't reach Modrinth or CurseForge. Check your connection and try again.")
                    .level(Level::Error)
                    .send();
                return;
            }
        };

        let java_major = oneclient_core::required_java_major(&state, offer.target.id)
            .await
            .ok()
            .flatten();

        let selected = sources[0].id;
        self.write_release_migration(move |prompt| {
            *prompt = Some(ReleaseMigrationPrompt {
                key: offer.release.key(),
                target: offer.target,
                java_major,
                sources,
                selected,
                plans,
                origin: PromptOrigin::Manual,
            });
        });
    }

    pub fn queue_release_migration(&self, target_cluster_id: i64) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let Ok(target) = state.clusters.get(target_cluster_id).await else { return };
            let key = ReleaseTarget {
                mc_version: target.mc_version,
                loader: target.mc_loader,
            }
            .key();

            actions.write_release_migration(|prompt| *prompt = None);
            actions.edit_settings(move |settings| {
                if !settings.pending_release_migrations.contains(&key) {
                    settings.pending_release_migrations.push(key);
                }
            });
            actions.check_release_migration();
        });
    }

    pub fn process_release_waitlist(&self) {
        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let content = state.services.content();

            let installs = match process_waitlist(
                state.bundles.as_ref(),
                |cluster_id| oneclient_core::game::is_running(&state, cluster_id),
                &content,
            )
            .await
            {
                Ok(installs) => installs,
                Err(err) => {
                    tracing::warn!(error = %err, "release migration waitlist check failed");
                    return;
                }
            };

            if installs.is_empty() {
                return;
            }

            for install in &installs {
                let verb = if install.names.len() == 1 { "supports" } else { "support" };
                actions
                    .notify(format!("New {} builds added", install.mc_version))
                    .body(format!(
                        "{} now {verb} {} and {} added to {}.",
                        install.names.join(", "),
                        install.mc_version,
                        if install.names.len() == 1 { "was" } else { "were" },
                        install.cluster_name
                    ))
                    .send();
            }

            super::super::invalidate_cluster_queries().await;
        });
    }

    pub fn dismiss_release_migration(&self) {
        let mut key = None;
        self.write_release_migration(|prompt| {
            key = prompt
                .take()
                .filter(|prompt| prompt.origin == PromptOrigin::NewRelease)
                .map(|prompt| prompt.key);
        });
        if let Some(key) = key {
            self.remove_pending_release_migrations(vec![key]);
        }
    }

    pub fn migrate_release_packages(&self, packages: Vec<ReleaseMigrationPackage>) {
        let mut taken = None;
        self.write_release_migration(|prompt| taken = prompt.take());
        let Some(prompt) = taken else { return };
        if prompt.origin != PromptOrigin::Simulated {
            self.remove_pending_release_migrations(vec![prompt.key.clone()]);
        }

        if packages.is_empty() {
            return;
        }

        let chosen: HashSet<String> = packages
            .iter()
            .map(|package| package.source_hash.clone())
            .collect();
        let (dependencies, unavailable): (Vec<_>, Vec<_>) = match prompt.plans.get(&prompt.selected) {
            Some(ReleasePlanState::Ready(plan)) => (
                plan.dependencies
                    .iter()
                    .filter(|dependency| dependency.is_needed_by(&chosen))
                    .map(|dependency| (dependency.clone(), dependency.enabled_for(&packages)))
                    .collect(),
                plan.unavailable.clone(),
            ),
            _ => (Vec::new(), Vec::new()),
        };

        let actions = self.clone();
        spawn_forever(async move {
            let Ok(state) = launcher::state() else { return };
            let content = state.services.content();
            let target = prompt.target;
            let total = packages.len() + dependencies.len();

            let session = oneclient_events::GroupedProgressSession::start(
                &state.services.events,
                format!("Migrating to {}", target.name),
            );
            session.expect(
                oneclient_events::TaskCategory::Packages,
                total as u64,
                total as u64,
            );

            let mut migrated = 0usize;
            for (dependency, enabled) in &dependencies {
                let child = session.child(
                    dependency.project.name.clone(),
                    1,
                    oneclient_events::TaskCategory::Packages,
                );
                match apply_release_migration_dependency(target.id, dependency, *enabled, Some(&child), &content).await {
                    Ok(_) => migrated += 1,
                    Err(err) => tracing::warn!(
                        dependency = %dependency.project.name,
                        error = %err,
                        "release migration failed for dependency"
                    ),
                }
                child.finish();
            }

            for package in &packages {
                let child = session.child(
                    package.display_name.clone(),
                    1,
                    oneclient_events::TaskCategory::Packages,
                );
                match apply_release_migration_package(target.id, package, Some(&child), &content).await {
                    Ok(_) => migrated += 1,
                    Err(err) => tracing::warn!(
                        package = %package.display_name,
                        error = %err,
                        "release migration failed for package"
                    ),
                }
                child.finish();
            }

            if let Err(err) = add_to_waitlist(target.id, &unavailable, &content).await {
                tracing::warn!(error = %err, "could not add packages without a build to the migration waitlist");
            }

            let failed = total - migrated;
            let session_id = session.detach();
            let spec = Some(NotificationSpec {
                title: format!("Migrated to {}", target.name),
                body: if failed == 0 {
                    format!("{migrated} package{} copied into {}", if migrated == 1 { "" } else { "s" }, target.name)
                } else {
                    format!("{migrated} of {total} packages copied, {failed} failed")
                },
                level: if failed == 0 { Level::Info } else { Level::Error },
                icon: Some(IconType::DownloadCloud02),
                progress: None,
                actions: Vec::new(),
            });

            actions.with_engine(|app| {
                app.notifications
                    .finish_grouped_as_actions(&mut app.inbox, session_id, spec);
            });

            super::super::invalidate_cluster_queries().await;
        });
    }
}
