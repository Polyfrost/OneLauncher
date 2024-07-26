//! **OneLauncher Store**
//!
//! Core state and storage management for the launcher, managing all states.

use std::sync::Arc;
use std::time::Duration;

use notify::RecommendedWatcher;
use notify_debouncer_mini::Debouncer;
use tokio::sync::{OnceCell, RwLock, Semaphore};

use crate::proxy::send::{init_ingress_internal, send_ingress, send_offline};
use crate::utils::http;
use crate::utils::http::{FetchSemaphore, IoSemaphore};

mod dirs;
pub use self::dirs::*;

mod minecraft;
pub use self::minecraft::*;

mod credentials;
pub use self::credentials::*;

mod discord;
pub use self::discord::*;

mod metadata;
pub use self::metadata::*;

mod settings;
pub use self::settings::*;

mod java_versions;
pub use self::java_versions::*;

mod processor;
pub use self::processor::*;

mod clusters;
pub use self::clusters::*;

mod package;
pub use self::package::*;

mod oneconfig;
pub use self::oneconfig::*;

/// The static [`OnceCell<RwLock<State>>`] for storing the global runtime launcher state.
static LAUNCHER_STATE: OnceCell<RwLock<State>> = OnceCell::const_new();

/// The public OneLauncher state structure.
pub struct State {
	/// Handles a boolean to check if the launcher is connected to the internet
	pub offline: RwLock<bool>,
	/// Handles core launcher directory and files
	pub directories: Directories,

	/// Semaphore used to handle concurrent network requests
	pub fetch_semaphore: FetchSemaphore,
	/// Maximum number of semaphore permits to limit the amount of network requests at one time
	pub fetch_semaphore_max: RwLock<u32>,
	/// Semaphore used to handle concurrent IO requests
	pub io_semaphore: IoSemaphore,
	/// Maximum number of semaphore permits to limit the amount of IO requests at one time
	pub io_semaphore_max: RwLock<u32>,

	/// Handles launcher metadata via [`interpulse`]
	pub metadata: RwLock<Metadata>,
	/// Handles launcher configuration and settings
	pub settings: RwLock<Settings>,
	/// Handles minecraft auth flows and users
	pub users: RwLock<MinecraftState>,
	/// Handles processes
	pub processor: RwLock<Processor>,
	/// Handles OneConfig integration with [`async-tungstenite`]
	pub oneconfig: RwLock<Option<OneConfig>>,
	/// Handles secure credential management with [`iota_stronghold`].
	pub credentials: RwLock<Option<Credentials>>,
	/// Handles clusters
	pub(crate) clusters: RwLock<Clusters>,
	/// Handles internal ingress processes
	pub(crate) ingress_processor: RwLock<IngressProcessor>,
	/// Handles file system watching for cluster manager
	pub(crate) watcher: RwLock<Debouncer<RecommendedWatcher>>,
	/// Handles Discord rich prescence
	pub discord_rpc: DiscordRPC,
}

impl State {
	/// Get the current global launcher state (or initialize it)
	pub async fn get() -> crate::Result<Arc<tokio::sync::RwLockReadGuard<'static, Self>>> {
		Ok(Arc::new(
			LAUNCHER_STATE
				.get_or_try_init(Self::initialize)
				.await?
				.read()
				.await,
		))
	}

	/// Get and lock writing to the current global launcher state (or initalize it)
	pub async fn get_and_write() -> crate::Result<tokio::sync::RwLockWriteGuard<'static, Self>> {
		Ok(LAUNCHER_STATE
			.get_or_try_init(Self::initialize)
			.await?
			.write()
			.await)
	}

	/// Initializes the core OneLauncher state fully.
	#[tracing::instrument]
	async fn initialize() -> crate::Result<RwLock<State>> {
		let ingress =
			init_ingress_internal(crate::IngressType::Initialize, 100.0, "initializing state")
				.await?;

		let settings = Settings::initialize(&Directories::init_settings_file()?).await?;
		let directories = Directories::initalize(&settings)?;
		send_ingress(&ingress, 10.0, None).await?;
		let mut watcher = crate::utils::watcher::initialize_watcher().await?;
		let fetch_semaphore =
			FetchSemaphore(RwLock::new(Semaphore::new(settings.max_async_fetches)));
		let io_semaphore = IoSemaphore(RwLock::new(Semaphore::new(
			settings.max_async_io_operations,
		)));
		send_ingress(&ingress, 10.0, None).await?;

		let is_offline = !http::check_internet_connection(3).await;

		// TODO: Make this run in the background, this delays launcher startup by a couple seconds
		let metadata_in =
			Metadata::initialize(&directories, !is_offline, &io_semaphore, &fetch_semaphore);
		let clusters_in = Clusters::initialize(&directories, &mut watcher);
		let users_in = MinecraftState::initialize(&directories, &io_semaphore);
		let (metadata, clusters, users) = crate::ingress_join! {
			Some(&ingress), 70.0, Some("loading core");
			metadata_in,
			clusters_in,
			users_in,
		}?;

		let ingress_processor = IngressProcessor::new();
		let discord_rpc = DiscordRPC::initialize(is_offline || settings.disable_discord).await?;
		if !settings.disable_discord && !is_offline {
			let _ = discord_rpc.apply_activity("Idling...", true).await;
		}

		let processor = Processor::new();

		Self::status_loop();
		send_ingress(&ingress, 10.0, None).await?;

		Ok::<RwLock<Self>, crate::Error>(RwLock::new(Self {
			offline: RwLock::new(is_offline),
			directories,
			fetch_semaphore,
			fetch_semaphore_max: RwLock::new(settings.max_async_fetches as u32),
			io_semaphore,
			io_semaphore_max: RwLock::new(settings.max_async_io_operations as u32),
			metadata: RwLock::new(metadata),
			settings: RwLock::new(settings),
			users: RwLock::new(users),
			processor: RwLock::new(processor),
			oneconfig: RwLock::new(None),
			credentials: RwLock::new(None),
			clusters: RwLock::new(clusters),
			ingress_processor: RwLock::new(ingress_processor),
			watcher: RwLock::new(watcher),
			discord_rpc,
		}))
	}

	/// Initalizes a "game loop" which checks for updates and if we are online.
	pub fn status_loop() {
		tokio::task::spawn(async {
			loop {
				let state = Self::get().await;
				if let Ok(state) = state {
					let _ = state.check_status().await;
				}

				tokio::time::sleep(Duration::from_secs(10)).await;
			}
		});
	}

	/// Updates all data if we are connected to the internet.
	pub fn update() {
		tokio::task::spawn(async {
			if let Ok(state) = crate::State::get().await {
				if !*state.offline.read().await {
					let version_up = Clusters::update_versions();
					let meta_up = Metadata::update();
					let package_up = Clusters::update_packages();

					let _ = tokio::join!(version_up, meta_up, package_up);
				}
			}
		});
	}

	/// Synchronizes data that can change outside of our control.
	#[tracing::instrument]
	pub async fn sync() -> crate::Result<()> {
		let state = Self::get().await?;
		let sync_settings = async {
			let state = Arc::clone(&state);
			tokio::spawn(async move {
				let reader = state.settings.read().await;
				reader.sync(&state.directories.settings_file()).await?;
				Ok::<_, crate::Error>(())
			})
			.await?
		};

		let sync_clusters = async {
			let state = Arc::clone(&state);

			tokio::spawn(async move {
				let clusters = state.clusters.read().await;

				clusters.sync().await?;
				Ok::<_, crate::Error>(())
			})
			.await?
		};

		tokio::try_join!(sync_settings, sync_clusters)?;
		Ok(())
	}

	/// Resets the IO [`Semaphore`] to clear all tasks.
	/// # This should ONLY be called when we can ensure
	/// # that there are no current tasks running!!
	pub async fn reset_io_semaphore(&self) {
		let settings = self.settings.read().await;
		let mut io_semaphore = self.io_semaphore.0.write().await;
		let mut total_permits = self.io_semaphore_max.write().await;
		// block ALL semaphore permits - this can be destructive
		let _ = io_semaphore.acquire_many(*total_permits).await;

		io_semaphore.close();
		*total_permits = settings.max_async_io_operations as u32;
		*io_semaphore = Semaphore::new(settings.max_async_io_operations);
	}

	/// Resets the Fetch [`Semaphore`] to clear all tasks.
	/// # This should ONLY be called when we can ensure
	/// # that there are no current tasks running!!
	pub async fn reset_fetch_semaphore(&self) {
		let settings = self.settings.read().await;
		let mut fetch_semaphore = self.fetch_semaphore.0.write().await;
		let mut total_permits = self.fetch_semaphore_max.write().await;
		// block ALL semaphore permits - this can be destructive
		let _ = fetch_semaphore.acquire_many(*total_permits).await;

		fetch_semaphore.close();
		*total_permits = settings.max_async_io_operations as u32;
		*fetch_semaphore = Semaphore::new(settings.max_async_fetches);
	}

	/// Checks if we are online used in a loop.
	pub async fn check_status(&self) -> crate::Result<()> {
		let is_online = http::check_internet_connection(3).await;
		let mut offline = self.offline.write().await;
		if *offline != is_online {
			return Ok(());
		}

		send_offline(!is_online).await?;
		*offline = !is_online;
		Ok(())
	}

	/// Determine if the global state is initalized or not (wrapper)
	pub fn initalized() -> bool {
		LAUNCHER_STATE.initialized()
	}
}
