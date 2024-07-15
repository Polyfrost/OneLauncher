//! Proxy for Tauri/CLI specific features and communication
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{OnceCell, RwLock};
use uuid::Uuid;

use crate::store::{ClusterPath, IngressProcessor};

pub mod send;
pub mod utils;
pub use utils::*;

static PROXY_STATE: OnceCell<Arc<ProxyState>> = OnceCell::const_new();
pub struct ProxyState {
	#[cfg(feature = "tauri")]
	pub app: tauri::AppHandle,
	pub ingress_feeds: RwLock<HashMap<Uuid, Ingress>>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Ingress {
	pub ingress_uuid: Uuid,
	pub message: String,
	pub ingress_type: IngressType,
	pub total: f64,
	pub current: f64,
	#[serde(skip)]
	pub last_sent: f64,
	#[cfg(feature = "cli")]
	#[serde(skip)]
	pub cli: indicatif::ProgressBar,
}

#[derive(Serialize, Debug, Clone)]
pub struct IngressId(Uuid);

impl Drop for IngressId {
	fn drop(&mut self) {
		let ingress_uuid = self.0;
		tokio::spawn(async move {
			if let Ok(proxy_state) = ProxyState::get().await {
				let mut ingress_feeds = proxy_state.ingress_feeds.write().await;
				#[cfg(any(feature = "tauri", feature = "cli"))]
				if let Some(ingress) = ingress_feeds.remove(&ingress_uuid) {
					#[cfg(feature = "cli")]
					{
						let cli = ingress.cli;
						cli.finish();
					}

					#[cfg(feature = "tauri")]
					{
						let _ingress_feed_uuid = ingress.ingress_uuid;
						let event = ingress.ingress_type.clone();
						let fraction = ingress.current / ingress.total;

						use tauri::Manager;
						let _ = proxy_state.app.emit(
							"ingress",
							IngressPayload {
								fraction: None,
								message: "Complete".to_string(),
								event,
								ingress_uuid,
							},
						);

						tracing::trace!("exited at {fraction} for ingress: {:?}", ingress_uuid);
					}
				}

				#[cfg(not(any(feature = "tauri", feature = "cli")))]
				ingress_feeds.remove(&ingress_uuid);
			}

			if crate::State::initalized() {
				let _ = IngressProcessor::finish(
					crate::store::IngressProcessType::IngressFeed,
					ingress_uuid,
				)
				.await;
			}
		});
	}
}

impl ProxyState {
	#[cfg(feature = "tauri")]
	pub async fn initialize(app: tauri::AppHandle) -> crate::Result<Arc<Self>> {
		PROXY_STATE
			.get_or_try_init(|| async {
				Ok(Arc::new(Self {
					app,
					ingress_feeds: RwLock::new(HashMap::new()),
				}))
			})
			.await
			.cloned()
	}

	#[cfg(not(feature = "tauri"))]
	pub async fn initialize() -> crate::Result<Arc<Self>> {
		PROXY_STATE
			.get_or_try_init(|| async {
				Ok(Arc::new(Self {
					ingress_feeds: RwLock::new(HashMap::new()),
				}))
			})
			.await
			.cloned()
	}

	#[cfg(feature = "tauri")]
	pub async fn get_main_window() -> crate::Result<tauri::WebviewWindow> {
		use tauri::Manager;
		let value = Self::get().await?;
		Ok(value.app.get_webview_window("main").unwrap())
	}

	#[cfg(feature = "tauri")]
	pub async fn get() -> crate::Result<Arc<Self>> {
		Ok(PROXY_STATE.get().ok_or(ProxyError::NotInitialized)?.clone())
	}

	#[cfg(not(feature = "tauri"))]
	pub async fn get() -> crate::Result<Arc<Self>> {
		Self::initialize().await
	}

	pub async fn list_ingress_feeds() -> crate::Result<HashMap<Uuid, Ingress>> {
		let value = Self::get().await?;
		let read = value.ingress_feeds.read().await;

		let mut display_list: HashMap<Uuid, Ingress> = HashMap::new();
		for (uuid, ingress) in read.iter() {
			display_list.insert(*uuid, ingress.clone());
		}

		Ok(display_list)
	}
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, PartialEq, Eq)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum IngressType {
	Initialize,
	DownloadJava {
		version: u32,
	},
	DownloadLoader {
		cluster_path: PathBuf,
		cluster_name: String,
	},
	SyncCluster {
		cluster_path: PathBuf,
		cluster_name: String,
	},
	CopyCluster {
		import: PathBuf,
		cluster_name: String,
	},
	SyncConfig {
		new_path: PathBuf,
	},
	Archival {
		cluster_path: PathBuf,
		cluster_name: String,
	},
	DownloadPackage {
		cluster_path: PathBuf,
		package_name: String,
		icon: Option<PathBuf>,
		package_id: Option<String>,
		package_version: Option<String>,
	},
	DownloadPack {
		cluster_path: PathBuf,
		package_name: String,
		icon: Option<String>,
		package_version: String,
	},
}

#[derive(Serialize, Clone)]
pub struct IngressPayload {
	pub event: IngressType,
	pub ingress_uuid: Uuid,
	pub fraction: Option<f64>,
	pub message: String,
}

#[derive(Serialize, Clone)]
pub struct OfflinePayload {
	pub offline: bool,
}

#[derive(Serialize, Clone)]
pub struct MessagePayload {
	pub message: String,
}

#[derive(Serialize, Clone)]
#[serde(tag = "event")]
pub enum InternetPayload {
	InstallPackage { id: String },
	InstallPack { id: String },
	InstallPath { path: PathBuf },
}

#[derive(Serialize, Clone)]
pub struct ProcessPayload {
	pub uuid: Uuid,
	pub pid: u32,
	pub event: ProcessPayloadType,
	pub message: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ProcessPayloadType {
	Started,
	Modified,
	Finished,
}

#[derive(Serialize, Clone)]
pub struct ClusterPayload {
	pub uuid: Uuid,
	pub cluster_path: ClusterPath,
	pub path: PathBuf,
	pub name: String,
	pub event: ClusterPayloadType,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ClusterPayloadType {
	Created,
	Inserted,
	Synced,
	Edited,
	Deleted,
}

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
	#[error("event state was not initialized!")]
	NotInitialized,

	#[error("non-existant ingress of key: {0}")]
	NoIngressFound(Uuid),

	#[cfg(feature = "tauri")]
	#[error("Tauri error: {0}")]
	TauriError(#[from] tauri::Error),
}
