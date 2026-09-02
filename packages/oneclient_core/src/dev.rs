use std::collections::HashMap;
use std::sync::Arc;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::mpsc;
use uuid::Uuid;

use oneclient_content::packages::provider::PackageProviderRegistry;
use oneclient_net::RequestClient;
use oneclient_events::{
	Event, EventBus, GameEvent, GroupedProgressEvent, Level, Notification, ProgressEvent,
};

use crate::{
	LauncherResult, LauncherServices, LauncherState,
};

struct GroupedSessionUi {
	title: String,
	parent: ProgressBar,
	children: HashMap<Uuid, ProgressBar>,
	child_bytes: HashMap<Uuid, (u64, u64)>,
}

fn spawn_notification_handler(mut rx: oneclient_events::EventReceiver) {
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

        while let Some(event) = rx.recv().await {
            match event {
                Event::Notification(Notification::Message(message)) => {
                    let (title, body) = (message.title, message.body);
                    mp.suspend(|| match message.level {
                        Level::Info => tracing::info!(%title, %body),
                        Level::Error => tracing::error!(%title, %body),
                    });
                }
                Event::Progress(ProgressEvent::Update { id, label, current, total }) => {
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
                Event::Progress(ProgressEvent::Complete { id, title, body }) => {
                    if let Some(pb) = progress_bars.remove(&id) {
                        pb.finish_with_message(format!("{title}: {body}"));
                    } else {
                        mp.suspend(|| tracing::info!(%title, %body));
                    }
                }
                Event::Progress(ProgressEvent::Grouped(event)) => match event {
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
                    GroupedProgressEvent::Expect { .. } => {}
                    GroupedProgressEvent::AddChild {
                        session_id,
                        child_id,
                        label,
                        total,
                        ..
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
                Event::Signal(_) => {}
                Event::Game(GameEvent::Stage { cluster_id, stage }) => {
                    mp.suspend(|| tracing::info!(cluster_id, ?stage, "game stage"));
                }
                Event::Game(GameEvent::Log { line, .. }) => {
                    mp.suspend(|| tracing::info!("[game] {line}"));
                }
                Event::Game(GameEvent::Failed { cluster_id, message }) => {
                    mp.suspend(|| tracing::error!(cluster_id, "launch failed: {message}"));
                }
                Event::Game(GameEvent::Crashed(crash)) => {
                    mp.suspend(|| {
                        tracing::error!(
                            cluster_id = crash.cluster_id,
                            fixes = crash.fixes.len(),
                            "{}",
                            crash.title
                        );
                    });
                }
                // No TTY dialog here answer `None` so the waiting task fails
                // cleanly instead of hanging on a prompt nobody can answer
                Event::Notification(Notification::Prompt(request)) => {
                    let choices: Vec<&str> = request.choices.iter().map(|c| c.id).collect();
                    mp.suspend(|| {
                        tracing::warn!(
                            "{}: {} | choices: {choices:?}",
                            request.title,
                            request.body
                        );
                        tracing::warn!("prompts are not interactive in dev; dismissing");
                    });
                    let _ = request.reply.send(None);
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

    LauncherState::new(EventBus::new(tx)).await
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
	oneclient_common::paths::set_launcher_dir(root.clone());

	let (tx, rx) = mpsc::unbounded_channel();
	spawn_notification_handler(rx);

	let db = oneclient_db::connect(root.join("example.db")).await?;
	let settings = crate::settings::store::load_settings(None).await;

	let services = LauncherServices {
		events: EventBus::new(tx),
		requester: RequestClient::new(oneclient_net::NetConfig::default())?,
		db,
		packages: PackageProviderRegistry::new(),
	};
	let auth = Arc::new(oneclient_auth::AuthService::with_store(
		Default::default(),
		services.requester.clone(),
		services.events.clone(),
	));

	let clusters = crate::clusters::ClusterManager::new(services.db.clone());

	let java = oneclient_java::JavaService::new(
		Arc::new(crate::java_store::SqlJavaStore::new(services.db.clone())),
		services.requester.clone(),
		services.events.clone(),
	);

	Ok(Arc::new(LauncherState {
		services,
		auth,
		java,
		clusters,
		settings: parking_lot::RwLock::new(settings),
		metadata: tokio::sync::Mutex::new(oneclient_mc::MetadataStore::new()),
		bundles: Arc::new(oneclient_content::bundles::BundlesManager::new()),
		versions: Arc::new(crate::versions::VersionsManager::new()),
		images: crate::images::ImageCacheStore::new(),
		games: crate::game::GameProcessManager::new(),
		discord: oneclient_discord::DiscordRpc::spawn(false),
	}))
}

pub async fn ephemeral_services() -> LauncherResult<LauncherServices> {
	let root = ephemeral_root().await?;
	oneclient_common::paths::set_launcher_dir(root.clone());
	let db = oneclient_db::connect(root.join("example.db")).await?;
	let (tx, rx) = mpsc::unbounded_channel();
	spawn_notification_handler(rx);

	Ok(LauncherServices {
		events: EventBus::new(tx),
		requester: RequestClient::new(oneclient_net::NetConfig::default())?,
		db,
		packages: PackageProviderRegistry::new(),
	})
}

pub async fn seed_bundle_archive(
	state: &LauncherState,
	manifest: oneclient_content::bundles::BundleManifest,
) -> LauncherResult<()> {
	let disk_path = format!("bundles/{}.mrpack", manifest.name);
	let loader = manifest.loader as i64;

	oneclient_db::dao::bundle::upsert_bundle(
		&state.services.db,
		oneclient_db::models::NewBundle {
			remote_path: &disk_path,
			mc_version: &manifest.mc_version,
			mc_loader: loader,
			file_name: &format!("{}.mrpack", manifest.name),
			name: Some(&manifest.name),
			version_id: Some(&manifest.version_id),
			category: Some(&manifest.category),
			loader_version: Some(&manifest.loader_version),
			disk_path: &disk_path,
			hidden: false,
			etag: None,
			synced_at: None,
		},
	)
	.await?;

	let path = oneclient_common::paths::launcher_dir()?.join(&disk_path);
	state.bundles.cache_archive_manifest(path, manifest).await;

	Ok(())
}
