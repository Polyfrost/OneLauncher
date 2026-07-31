use std::sync::Arc;

use oneclient_db::DbPool;
use tokio::sync::Mutex;

use oneclient_auth::AuthService;
use crate::clusters::ClusterManager;
use oneclient_content::bundles::BundlesManager;
use oneclient_discord::DiscordRpc;
use oneclient_net::RequestClient;
use crate::images::ImageCacheStore;
use oneclient_java::JavaService;
use oneclient_events::EventBus;
use oneclient_mc::MetadataStore;
use oneclient_content::packages::provider::PackageProviderRegistry;
use oneclient_common::paths;
use crate::settings::{store, LauncherSettings};
use crate::versions::VersionsManager;
use crate::LauncherResult;

#[derive(Clone)]
pub struct LauncherServices {
	pub events: EventBus,
	pub requester: RequestClient,
	pub db: DbPool,
	pub packages: PackageProviderRegistry,
}

pub struct LauncherState {
	pub services: LauncherServices,
	pub settings: parking_lot::RwLock<LauncherSettings>,
	pub auth: Arc<AuthService>,
	pub java: JavaService,
	pub clusters: ClusterManager,
	pub metadata: Mutex<MetadataStore>,
	pub bundles: Arc<BundlesManager>,
	pub versions: Arc<VersionsManager>,
	pub images: ImageCacheStore,
	pub games: crate::game::GameProcessManager,
	pub discord: DiscordRpc,
}

impl LauncherServices {
	/// What the content crate needs: db, client, bus and the provider registry.
	///
	/// Unlike [`LauncherServices::mc`] this is the whole of `LauncherServices`; the
	/// packages and bundles code uses all four fields.
	#[must_use]
	pub fn content(&self) -> oneclient_content::ContentCtx {
		oneclient_content::ContentCtx::new(
			self.db.clone(),
			self.requester.clone(),
			self.events.clone(),
			self.packages.clone(),
		)
	}

	/// The subset the Minecraft content crate needs: a client and a bus.
	///
	/// Both are refcounted handles, so this is cheap enough to build per call
	/// rather than store alongside the fields it is made of.
	#[must_use]
	pub fn mc(&self) -> oneclient_mc::McCtx {
		oneclient_mc::McCtx::new(self.requester.clone(), self.events.clone())
	}
}

impl LauncherState {
	/// Builds the launcher's services and returns the handle.
	///
	/// Construction has no side effects beyond opening the database and reading
	/// settings: the background startup work is [`run_startup_tasks`], which the
	/// caller kicks off when it is ready. That split is why the state can be
	/// constructed twice, or in a test only once.
    #[tracing::instrument(skip(events))]
	pub async fn new(events: EventBus) -> LauncherResult<Arc<Self>> {
        let services = LauncherServices {
			events,
			db: oneclient_db::connect(paths::database_file()?).await?,
			// Built with defaults because loading settings needs the event bus,
			// which is already inside `services`. The user's endpoints and keys
			// are pushed in immediately below, before anything makes a request.
			requester: RequestClient::new(oneclient_net::NetConfig::default())?,
			packages: PackageProviderRegistry::new(),
		};

        let settings = store::load_settings(Some(&services.events)).await;
		services
			.requester
			.set_config(crate::settings::net_config(&settings));
        let auth = Arc::new(
			AuthService::load(services.requester.clone(), services.events.clone()).await?,
		);
        let java = JavaService::new(
			std::sync::Arc::new(crate::java_store::SqlJavaStore::new(services.db.clone())),
			services.requester.clone(),
			services.events.clone(),
		);
        let discord = DiscordRpc::spawn(settings.discord_enabled);

		let clusters = ClusterManager::new(services.db.clone());

		let state = Arc::new(Self {
			services,
			settings: parking_lot::RwLock::new(settings),
			auth,
			java,
			clusters,
			metadata: Mutex::new(MetadataStore::new()),
			bundles: Arc::new(BundlesManager::new()),
			versions: Arc::new(VersionsManager::from_cache().await),
			images: ImageCacheStore::new(),
			games: crate::game::GameProcessManager::new(),
			discord,
		});

		Ok(state)
	}
}

/// Reconstructs anything missing from disk, adopts orphaned game sessions, then
/// syncs the remote catalogs.
///
/// Split out so constructing a `LauncherState` is just construction. The caller
/// decides when, and whether, this runs.
pub fn run_startup_tasks(state: &Arc<LauncherState>) {
	let background = Arc::clone(state);
	tokio::spawn(async move {
			let recovery = match crate::recovery::reconstruct_from_disk(&background).await {
				Ok(report) => report,
				Err(err) => {
					tracing::error!("disk recovery failed: {err:#}");
					crate::recovery::RecoveryReport::default()
				}
			};

			crate::game::recover_sessions(&background).await;

			let content = background.services.content();
			let (versions_res, bundles_res) = tokio::join!(
				background.versions.sync(&background.services),
				background.bundles.sync(&content),
			);
			if let Err(err) = versions_res {
				tracing::error!("versions manifest sync failed: {err:#}");
			}
			if let Err(err) = bundles_res {
				tracing::error!("bundle catalog sync failed: {err:#}");
			}

			if recovery.did_recover()
				&& let Err(err) = crate::recovery::restore_bundle_tracking(&background).await
			{
				tracing::warn!("bundle tracking restore failed: {err:#}");
			}

			if let Err(err) = crate::clusters::apply_remote_migrations(&background).await {
				tracing::error!("cluster migrations failed: {err:#}");
			}

			if let Err(err) = crate::clusters::ensure_from_versions(&background).await {
				tracing::error!("versions cluster provisioning failed: {err:#}");
			} else {
				background.services.events.signal(oneclient_events::Signal::ClustersChanged);
			}

			// Last, so it runs against a database that recovery, bundle tracking
			// and provisioning have all finished settling. Judging an artifact
			// unused before the rows that use it have been restored would evict
			// content that is very much still wanted.
			//
			// Only the row-driven half runs unattended. Deciding from the
			// filesystem which cached files nothing points at is the half that can
			// misread a half-reconstructed install, so it lives behind the Storage
			// settings page where the user sees what it proposes to delete.
			if let Err(err) =
				oneclient_content::packages::store::collect_unused_artifacts(&content).await
			{
				tracing::warn!("package cache cleanup failed: {err:#}");
			}

		background.services.events.signal(oneclient_events::Signal::SyncComplete);
	});
}
