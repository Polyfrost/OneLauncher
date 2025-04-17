use serde::Serialize;
use uuid::Uuid;

use crate::LauncherResult;
use crate::store::State;
use crate::store::ingress::{IngressId, IngressType, SubIngress};

#[onelauncher_macro::specta(with_event)]
#[derive(Serialize, Debug, Clone)]
pub struct IngressPayload {
	pub id: Uuid,
	pub message: String,
	pub ingress_type: IngressType,
	pub percent: Option<f64>,
	pub total: f64,
}

#[tracing::instrument]
pub async fn init_ingress(
	ingress_type: IngressType,
	message: &str,
	total: f64,
) -> LauncherResult<IngressId> {
	let state = State::get().await?;
	let processor = &state.ingress_processor;

	let id = processor
		.create(ingress_type, message.to_string(), total)
		.await?;

	Ok(id)
}

pub async fn init_ingress_opt(
	init: bool,
	ingress_type: IngressType,
	message: &str,
	total: f64,
) -> LauncherResult<Option<IngressId>> {
	if init {
		let id = init_ingress(ingress_type, message, total).await?;
		Ok(Some(id))
	} else {
		Ok(None)
	}
}

/// Sends an ingress update with the given increment and message
#[tracing::instrument]
pub async fn send_ingress_message(
	id: &IngressId,
	increment: f64,
	message: Option<&str>,
) -> LauncherResult<()> {
	if !State::initialized() {
		tracing::debug!("attempted to send ingress when state is not initialized");
		return Ok(());
	}

	let state = State::get().await?;
	let processor = &state.ingress_processor;

	processor
		.send(id, increment, message.map(String::from))
		.await?;
	Ok(())
}

/// Sends an ingress update with the given increment
pub async fn send_ingress(
	id: &IngressId,
	increment: f64,
) -> LauncherResult<()> {
	send_ingress_message(id, increment, None).await
}

/// Sends an ingress update with the given message (does not increment)
pub async fn set_ingress_message(
	id: &IngressId,
	message: &str,
) -> LauncherResult<()> {
	send_ingress_message(id, 0.0, Some(message)).await
}

#[async_trait::async_trait]
pub trait IngressSendExt {
	async fn send_ingress_message(&self, increment: f64, message: Option<&str>) -> LauncherResult<()>;
	async fn send_ingress(&self, increment: f64) -> LauncherResult<()>;
	async fn set_ingress_message(&self, message: &str) -> LauncherResult<()>;
}

#[async_trait::async_trait]
impl IngressSendExt for IngressId {
	async fn send_ingress_message(&self, increment: f64, message: Option<&str>) -> LauncherResult<()> {
		send_ingress_message(self, increment, message).await
	}

	async fn send_ingress(&self, increment: f64) -> LauncherResult<()> {
		send_ingress(self, increment).await
	}

	async fn set_ingress_message(&self, message: &str) -> LauncherResult<()> {
		set_ingress_message(self, message).await
	}
}

#[async_trait::async_trait]
impl IngressSendExt for Option<&IngressId> {
	async fn send_ingress_message(&self, increment: f64, message: Option<&str>) -> LauncherResult<()> {
		if let Some(id) = self {
			id.send_ingress_message(increment, message).await
		} else {
			Ok(())
		}
	}

	async fn send_ingress(&self, increment: f64) -> LauncherResult<()> {
		if let Some(id) = self {
			id.send_ingress(increment).await
		} else {
			Ok(())
		}
	}

	async fn set_ingress_message(&self, message: &str) -> LauncherResult<()> {
		if let Some(id) = self {
			id.set_ingress_message(message).await
		} else {
			Ok(())
		}
	}
}

#[async_trait::async_trait]
impl IngressSendExt for &SubIngress<'_> {
	async fn send_ingress_message(&self, increment: f64, message: Option<&str>) -> LauncherResult<()> {
		self.id.send_ingress_message(increment, message).await
	}

	async fn send_ingress(&self, increment: f64) -> LauncherResult<()> {
		self.id.send_ingress(increment).await
	}

	async fn set_ingress_message(&self, message: &str) -> LauncherResult<()> {
		self.id.set_ingress_message(message).await
	}
}

#[async_trait::async_trait]
impl IngressSendExt for Option<&SubIngress<'_>> {
	async fn send_ingress_message(&self, increment: f64, message: Option<&str>) -> LauncherResult<()> {
		if let Some(id) = self {
			id.send_ingress_message(increment, message).await
		} else {
			Ok(())
		}
	}

	async fn send_ingress(&self, increment: f64) -> LauncherResult<()> {
		if let Some(id) = self {
			id.send_ingress(increment).await
		} else {
			Ok(())
		}
	}

	async fn set_ingress_message(&self, message: &str) -> LauncherResult<()> {
		if let Some(id) = self {
			id.set_ingress_message(message).await
		} else {
			Ok(())
		}
	}
}

#[cfg(test)]
pub mod tests {
	use crate::api::ingress::{init_ingress, IngressSendExt};
	use crate::api::proxy::ProxyDynamic;
	use crate::initialize_core;
	use crate::store::CoreOptions;
	use crate::store::ingress::IngressType;
	use std::time::Duration;

	#[tokio::test]
	pub async fn create_and_update_ingress() -> crate::LauncherResult<()> {
		initialize_core(CoreOptions::default(), ProxyDynamic::new()).await?;

		let id = init_ingress(
			IngressType::Download {
				file_name: "Some-Mod-1.8.9.jar".into(),
			},
			"this is a test message",
			100.0,
		)
		.await?;
		tokio::time::sleep(Duration::from_millis(2350)).await;

		id.send_ingress(10.0).await?;
		tokio::time::sleep(Duration::from_millis(2500)).await;

		id.send_ingress(20.0).await?;
		tokio::time::sleep(Duration::from_millis(2500)).await;

		id.send_ingress(70.0).await?;
		tokio::time::sleep(Duration::from_millis(2500)).await;

		drop(id);

		Ok(())
	}
}
