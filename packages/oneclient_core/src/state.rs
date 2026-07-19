use std::collections::HashMap;
use std::sync::Arc;

use oneclient_db::DbPool;
use tokio::sync::{Mutex, OnceCell};

use crate::auth::{CredentialsStore, PendingBrowserLogin};
use crate::bundles::BundlesManager;
use crate::discord::DiscordRpc;
use crate::http::RequestClient;
use crate::images::ImageCacheStore;
use crate::java::JavaManager;
use crate::notification::NotificationService;
use crate::metadata::MetadataStore;
use crate::packages::provider::PackageProviderRegistry;
use crate::paths;
use crate::settings::{store, LauncherSettings};
use crate::versions::VersionsManager;
use crate::{LauncherError, LauncherResult};

static STATE: OnceCell<Arc<LauncherState>> = OnceCell::const_new();

#[derive(Clone)]
pub struct LauncherServices {
	pub notifier: NotificationService,
	pub requester: RequestClient,
	pub db: DbPool,
	pub packages: PackageProviderRegistry,
}

pub struct LauncherState {
	pub services: LauncherServices,
	pub settings: parking_lot::RwLock<LauncherSettings>,
	pub auth: Mutex<CredentialsStore>,
	pub microsoft_logins: Mutex<HashMap<String, PendingBrowserLogin>>,
	pub java: JavaManager,
	pub metadata: Mutex<MetadataStore>,
	pub bundles: Arc<BundlesManager>,
	pub versions: Arc<VersionsManager>,
	pub images: ImageCacheStore,
	pub games: crate::game::GameProcessManager,
	pub discord: DiscordRpc,
	pub provisioning: Mutex<()>,
}

impl LauncherState {
    #[tracing::instrument(skip(notifier))]
	pub async fn initialize(
		notifier: NotificationService,
	) -> LauncherResult<Arc<Self>> {
		if let Some(state) = STATE.get() {
			return Ok(Arc::clone(state));
		}

        let services = LauncherServices {
			notifier,
			db: oneclient_db::connect(paths::database_file()?).await?,
			requester: RequestClient::new()?,
			packages: PackageProviderRegistry::new(),
		};

        let settings = store::load_settings(Some(&services.notifier)).await;
        let auth = CredentialsStore::load().await?;
        let java = JavaManager;
        let discord = DiscordRpc::spawn(settings.discord_enabled);

		let state = Arc::new(Self {
			services,
			settings: parking_lot::RwLock::new(settings),
			auth: Mutex::new(auth),
			microsoft_logins: Mutex::new(HashMap::new()),
			java,
			metadata: Mutex::new(MetadataStore::new()),
			bundles: Arc::new(BundlesManager::new()),
			versions: Arc::new(VersionsManager::new()),
			images: ImageCacheStore::new(),
			games: crate::game::GameProcessManager::new(),
			discord,
			provisioning: Mutex::new(()),
		});

		STATE
			.set(Arc::clone(&state))
			.map_err(|_| LauncherError::AlreadyInitialized)?;

		let background = Arc::clone(&state);
		tokio::spawn(async move {
			let recovery = match crate::recovery::reconstruct_from_disk(&background).await {
				Ok(report) => report,
				Err(err) => {
					tracing::error!("disk recovery failed: {err:#}");
					crate::recovery::RecoveryReport::default()
				}
			};

			crate::game::recover_sessions(&background).await;

			let (versions_res, bundles_res) = tokio::join!(
				background.versions.sync(&background.services),
				background.bundles.sync(&background.services),
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
				background.services.notifier.invalidate_clusters();
			}
			background.services.notifier.sync_complete();
		});

		Ok(state)
	}

	pub fn get() -> LauncherResult<Arc<Self>> {
		STATE
			.get()
			.cloned()
			.ok_or(LauncherError::NotInitialized)
	}
}
