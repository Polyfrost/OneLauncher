use std::collections::HashMap;
use std::sync::Arc;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
	auth::CredentialsStore,
	http::RequestClient,
	notification::{
		GroupedProgressEvent, NotificationLevel, Notification, NotificationService,
	},
	packages::provider::PackageProviderRegistry,
	LauncherResult, LauncherServices, LauncherState,
};

struct GroupedSessionUi {
	title: String,
	parent: ProgressBar,
	children: HashMap<Uuid, ProgressBar>,
	child_bytes: HashMap<Uuid, (u64, u64)>,
}

fn spawn_notification_handler(mut rx: mpsc::UnboundedReceiver<Notification>) {
    tokio::spawn(async move {
        let mp = MultiProgress::new();
        let mut progress_bars: HashMap<Uuid, ProgressBar> = HashMap::new();
        let mut grouped_sessions: HashMap<Uuid, GroupedSessionUi> = HashMap::new();

        let flat_style = ProgressStyle::with_template(
            "{spinner:.green} {msg:<20} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        )
        .unwrap()
        .progress_chars("#>-");

        let parent_style = ProgressStyle::with_template(
            "{spinner:.green} {msg:<30} [{elapsed_precise}] [{wide_bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("#>-");

        let child_style = ProgressStyle::with_template(
            "  {spinner:.cyan} {msg:<38} [{bar:32.cyan/blue}] {bytes}/{total_bytes}",
        )
        .unwrap()
        .progress_chars("#>-");

        while let Some(notification) = rx.recv().await {
            match notification {
                Notification::Message { title, body, level } => {
                    mp.suspend(|| match level {
                        NotificationLevel::Info => tracing::info!(%title, %body),
                        NotificationLevel::Error => tracing::error!(%title, %body),
                    });
                }
                Notification::Progress { id, label, current, total } => {
                    if current >= total {
                        if let Some(pb) = progress_bars.remove(&id) {
                            pb.finish_with_message(format!("{label} Done!"));
                        }
                    } else {
                        let pb = progress_bars.entry(id).or_insert_with(|| {
                            let pb = mp.add(ProgressBar::new(total));
                            pb.set_style(flat_style.clone());
                            pb.set_message(label.clone());
                            pb
                        });

                        pb.set_length(total);
                        pb.set_position(current);
                    }
                }
                Notification::GroupedProgress(event) => match event {
                    GroupedProgressEvent::Start { session_id, title } => {
                        let parent = mp.add(ProgressBar::new(1));
                        parent.set_style(parent_style.clone());
                        parent.set_message(title.clone());

                        grouped_sessions.insert(
                            session_id,
                            GroupedSessionUi {
                                title,
                                parent,
                                children: HashMap::new(),
                                child_bytes: HashMap::new(),
                            },
                        );
                    }
                    GroupedProgressEvent::AddChild {
                        session_id,
                        child_id,
                        label,
                        total,
                    } => {
                        let Some(session) = grouped_sessions.get_mut(&session_id) else {
                            continue;
                        };

                        let child = ProgressBar::new(total);
                        child.set_style(child_style.clone());
                        child.set_message(label.clone());
                        mp.add(child.clone());

                        session.children.insert(child_id, child);
                        session.child_bytes.insert(child_id, (0, total));
                        refresh_grouped_parent(session);
                    }
                    GroupedProgressEvent::UpdateChild {
                        session_id,
                        child_id,
                        current,
                        total,
                    } => {
                        let Some(session) = grouped_sessions.get_mut(&session_id) else {
                            continue;
                        };

                        if let Some(child) = session.children.get(&child_id) {
                            child.set_length(total);
                            child.set_position(current);
                        }
                        session.child_bytes.insert(child_id, (current, total));
                        refresh_grouped_parent(session);
                    }
                    GroupedProgressEvent::SetChildPhase {
                        session_id,
                        child_id,
                        phase,
                    } => {
                        if let Some(session) = grouped_sessions.get(&session_id)
                            && let Some(child) = session.children.get(&child_id)
                        {
                            child.set_prefix(phase.label());
                        }
                    }
                    GroupedProgressEvent::FinishChild {
                        session_id,
                        child_id,
                    } => {
                        let Some(session) = grouped_sessions.get_mut(&session_id) else {
                            continue;
                        };

                        if let Some(child) = session.children.remove(&child_id) {
                            child.finish_and_clear();
                        }
                        session.child_bytes.remove(&child_id);
                        refresh_grouped_parent(session);
                    }
                    GroupedProgressEvent::End { session_id } => {
                        if let Some(session) = grouped_sessions.remove(&session_id) {
                            session.parent.finish_with_message(format!(
                                "{} - complete",
                                session.title
                            ));
                        }
                    }
                },
                Notification::InvalidateClusters => {}
                Notification::InvalidateJava => {}
                Notification::SyncComplete => {}
                Notification::GameStage { cluster_id, stage } => {
                    mp.suspend(|| tracing::info!(cluster_id, ?stage, "game stage"));
                }
                Notification::GameLog { line, .. } => {
                    mp.suspend(|| tracing::info!("[game] {line}"));
                }
                Notification::GameFailed { cluster_id, message } => {
                    mp.suspend(|| tracing::error!(cluster_id, "launch failed: {message}"));
                }
                Notification::Prompt { title, question, kind, .. } => {
                    mp.suspend(|| {
                        tracing::warn!("{title}: {question} | {kind:#?}");
                        tracing::warn!("prompt not implemented");
                    });
                }
            }
        }
    });
}

fn refresh_grouped_parent(session: &mut GroupedSessionUi) {
    let (current, total) = session
        .child_bytes
        .values()
        .fold((0u64, 0u64), |(cur, tot), (c, t)| (cur + c, tot + t));

    let total = total.max(1);
    session.parent.set_length(total);
    session.parent.set_position(current.min(total));
}

pub async fn initialize() -> LauncherResult<Arc<LauncherState>> {
    let (tx, rx) = mpsc::unbounded_channel();

    spawn_notification_handler(rx);

    LauncherState::initialize(NotificationService::new(tx)).await
}

async fn ephemeral_root() -> LauncherResult<std::path::PathBuf> {
	let path = std::env::current_dir()
		.map_err(crate::LauncherError::StdIoError)?
		.join("target")
		.join(format!("ephemeral-{}", Uuid::new_v4()));

	polyio::create_dir_all(&path).await?;
	
    Ok(path)
}

pub async fn ephemeral_state() -> LauncherResult<Arc<LauncherState>> {
	let root = ephemeral_root().await?;
	crate::paths::set_launcher_dir(root.clone());

	let (tx, rx) = mpsc::unbounded_channel();
	spawn_notification_handler(rx);

	let db = oneclient_db::connect(root.join("example.db")).await?;
	let settings = crate::settings::store::load_settings(None).await;

	Ok(Arc::new(LauncherState {
		services: LauncherServices {
			notifier: NotificationService::new(tx),
			requester: RequestClient::new()?,
			db,
			packages: PackageProviderRegistry::new(),
		},
		settings: parking_lot::RwLock::new(settings),
		auth: tokio::sync::Mutex::new(CredentialsStore::default()),
		java: crate::java::JavaManager,
		metadata: tokio::sync::Mutex::new(crate::metadata::MetadataStore::new()),
		bundles: Arc::new(crate::bundles::BundlesManager::new()),
		versions: Arc::new(crate::versions::VersionsManager::new()),
		images: crate::images::ImageCacheStore::new(),
		games: crate::game::GameProcessManager::new(),
	}))
}

pub async fn ephemeral_services() -> LauncherResult<LauncherServices> {
	let root = ephemeral_root().await?;
	crate::paths::set_launcher_dir(root.clone());
	let db = oneclient_db::connect(root.join("example.db")).await?;
	let (tx, rx) = mpsc::unbounded_channel();
	spawn_notification_handler(rx);

	Ok(LauncherServices {
		notifier: NotificationService::new(tx),
		requester: RequestClient::new()?,
		db,
		packages: PackageProviderRegistry::new(),
	})
}
