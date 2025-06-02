use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::api::proxy::event::try_send_event;
use crate::LauncherResult;
use crate::api::ingress::IngressPayload;

use super::State;

#[derive(Default)]
pub struct IngressProcessor {
	ingress_feeds: RwLock<HashMap<Uuid, Ingress>>,
}

#[onelauncher_macro::specta]
#[derive(Debug, thiserror::Error, Serialize)]
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

	pub async fn create(
		&self,
		ingress_type: IngressType,
		message: String,
		total: f64,
	) -> LauncherResult<IngressId> {
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

		self.send(&ingress_id, 0.0, None).await?;

		Ok(ingress_id)
	}

	pub async fn send(
		&self,
		id: &IngressId,
		increment: f64,
		message: Option<String>,
	) -> LauncherResult<()> {
		let mut feeds = self.ingress_feeds.write().await;
		let uuid = &id.0;
		let ingress = feeds.get_mut(uuid).ok_or(IngressError::NotFound)?;

		ingress.current += increment;
		if let Some(message) = message {
			ingress.message = message;
		}

		let payload = IngressPayload {
			id: uuid.to_owned(),
			message: ingress.message.clone(),
			ingress_type: ingress.ingress_type.clone(),
			percent: Some(ingress.current / ingress.total),
			total: ingress.total,
		};

		try_send_event(crate::api::proxy::event::LauncherEvent::Ingress(payload))
			.await?;

		Ok(())
	}

	pub async fn set_message(&self, id: &IngressId, message: String) -> LauncherResult<()> {
		self.send(id, 0.0, Some(message)).await
	}

	pub async fn remove(&self, uuid: Uuid) -> LauncherResult<Ingress> {
		let mut feeds = self.ingress_feeds.write().await;
		Ok(feeds.remove(&uuid).ok_or(IngressError::NotFound)?)
	}
}

#[derive(Debug, Clone)]
pub struct Ingress {
	pub id: Uuid,
	pub message: String,
	pub ingress_type: IngressType,
	pub total: f64,
	pub current: f64,
	pub last_sent: f64,
}

// #[onelauncher_macro::specta]
#[derive(Serialize, Deserialize, specta::Type, Debug, Clone)]
pub enum IngressType {
	Download { file_name: String },
	JavaPrepare,
	JavaCheck,
	JavaLocate,
	MinecraftDownload,
	PrepareCluster { cluster_name: String },
}

#[derive(Debug, Clone)]
pub struct IngressId(pub Uuid);

#[derive(Debug, Clone)]
pub struct SubIngress<'a> {
	pub id: &'a IngressId,
	pub total: f64,
}

// TODO: Rewrite the ingress system to use a proper parent <-> child system
impl<'a> SubIngress<'a> {
	#[must_use]
	pub const fn new(id: &'a IngressId, total: f64) -> Self {
		Self { id, total }
	}

	#[must_use]
	pub const fn from_sub(sub: &'a SubIngress<'_>, total: f64) -> Self {
		Self { id: sub.id, total }
	}
}

pub trait SubIngressExt {
	type Target<'sub>
	where
		Self: 'sub;

	type Total;

	fn ingress_total(&self) -> Self::Total;
	fn ingress_sub<F>(&self, total: F) -> Self::Target<'_>
	where
		F: FnOnce(f64) -> f64;
}

impl SubIngressExt for SubIngress<'_> {
	type Target<'sub>
		= SubIngress<'sub>
	where
		Self: 'sub;

	type Total = f64;

	fn ingress_total(&self) -> Self::Total {
		self.total
	}

	fn ingress_sub<F>(&self, total: F) -> Self::Target<'_>
	where
		F: FnOnce(f64) -> f64,
	{
		SubIngress::from_sub(self, total(self.total))
	}
}

impl SubIngressExt for Option<&SubIngress<'_>> {
	type Target<'sub>
		= Option<SubIngress<'sub>>
	where
		Self: 'sub;

	type Total = Option<f64>;

	fn ingress_total(&self) -> Self::Total {
		self.map(|sub| sub.total)
	}

	fn ingress_sub<F>(&self, total: F) -> Self::Target<'_>
	where
		F: FnOnce(f64) -> f64,
	{
		self.map(|sub| SubIngress::from_sub(sub, total(sub.total)))
	}
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

				let payload = IngressPayload {
					id: ingress.id,
					message: "Completed".into(),
					ingress_type: ingress.ingress_type,
					percent: None,
					total: ingress.total,
				};

				try_send_event(crate::api::proxy::event::LauncherEvent::Ingress(payload))
					.await?;

				tracing::trace!(
					"exited at {}% for ingress {:?}",
					ingress.current / ingress.total * 100.0,
					ingress_uuid
				);

				Ok(())
			}
			.await;

			if let Err(e) = result {
				tracing::error!("failed to finish ingress: {:?}", e);
			}
		});
	}
}
