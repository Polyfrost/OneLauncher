use std::collections::HashMap;

use serde::Serialize;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::LauncherResult;

use super::{proxy::ProxyState, State};

#[derive(Default)]
pub struct IngressProcessor {
	ingress_feeds: RwLock<HashMap<Uuid, Ingress>>,
}

#[derive(Debug, thiserror::Error)]
pub enum IngressError {
	#[error("ingress not found")]
	NotFound,
}

impl IngressProcessor {
	#[must_use]
	#[tracing::instrument]
	pub fn new() -> Self {
		let processor = Self {
			ingress_feeds: RwLock::new(HashMap::new()),
		};

		tracing::debug!("created ingress processor");

		processor
	}

	pub async fn create(&self, ingress_type: IngressType, message: String, total: f64) -> LauncherResult<IngressId> {
		let mut feeds = self.ingress_feeds.write().await;

		let uuid = Uuid::new_v4();
		let ingress_id = IngressId(uuid);

		let ingress = Ingress {
			id: uuid,
			message,
			ingress_type,
			total,
			current: 0.0,
			last_sent: 0.0,
		};

		feeds.insert(uuid, ingress);

		// Drop the write lock to prevent deadlock
		drop(feeds);

		self.send(&ingress_id, 0.0).await?;

		Ok(ingress_id)
	}

	pub async fn send(&self, id: &IngressId, increment: f64) -> LauncherResult<()> {
		let mut feeds = self.ingress_feeds.write().await;
		let uuid = &id.0;
		let ingress = feeds.get_mut(uuid).ok_or(IngressError::NotFound)?;

		let proxy = ProxyState::get()?;

		ingress.current += increment;
		proxy.send_ingress(IngressPayload {
			id: uuid.to_owned(),
			message: ingress.message.clone(),
			ingress_type: ingress.ingress_type.clone(),
			percent: Some(ingress.current / ingress.total),
			total: ingress.total,
		}).await?;

		Ok(())
	}

	pub async fn remove(&self, uuid: Uuid) -> LauncherResult<Ingress> {
		let mut feeds = self.ingress_feeds.write().await;
		Ok(feeds.remove(&uuid).ok_or(IngressError::NotFound)?)
	}
}


#[derive(Debug, Clone)]
pub struct IngressId(pub Uuid);

#[derive(Debug, Clone)]
pub struct Ingress {
	pub id: Uuid,
	pub message: String,
	pub ingress_type: IngressType,
	pub total: f64,
	pub current: f64,
	pub last_sent: f64,
}

#[onelauncher_macro::specta(with_event)]
#[derive(Serialize, Debug, Clone)]
pub struct IngressPayload {
	pub id: Uuid,
	pub message: String,
	pub ingress_type: IngressType,
	pub percent: Option<f64>,
	pub total: f64,
}

#[onelauncher_macro::specta]
#[derive(Serialize, Debug, Clone)]
pub enum IngressType {
	Download {
		file_name: String,
	},
	JavaCheck,
	JavaLocate
}

impl Drop for IngressId {
	#[clippy::has_significant_drop]
	fn drop(&mut self) {
		let ingress_uuid = self.0;
		tokio::spawn(async move {
			let result: LauncherResult<()> = async {
				let state = State::get().await?;
				let processor = &state.ingress_processor;

				let ingress = processor.remove(ingress_uuid).await?;

				let proxy = ProxyState::get()?;

				proxy.send_ingress(IngressPayload {
					id: ingress.id,
					message: "Completed".into(),
					ingress_type: ingress.ingress_type,
					percent: None,
					total: ingress.total,
				}).await?;

				tracing::trace!("exited at {}% for ingress {:?}", ingress.current / ingress.total * 100.0, ingress_uuid);

				Ok(())
			}.await;

			if let Err(e) = result {
				tracing::error!("failed to finish ingress: {:?}", e);
			}
		});
	}
}