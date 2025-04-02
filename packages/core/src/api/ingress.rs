use crate::{store::{ingress::{IngressId, IngressType}, State}, LauncherResult};

#[tracing::instrument]
pub async fn init_ingress(
	ingress_type: IngressType,
	message: &str,
	total: f64,
) -> LauncherResult<IngressId> {
	let state = State::get().await?;
	let processor = &state.ingress_processor;

	let id = processor.create(ingress_type, message.to_string(), total).await?;

	Ok(id)
}

#[tracing::instrument]
pub async fn send_ingress(id: &IngressId, increment: f64) -> LauncherResult<()> {
	let state = State::get().await?;
	let processor = &state.ingress_processor;

	processor.send(id, increment).await?;
	Ok(())
}

#[tracing::instrument]
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

#[tracing::instrument]
pub async fn send_ingress_opt(
	id: Option<&IngressId>,
	increment: f64,
) -> LauncherResult<()> {
	if let Some(id) = id {
		send_ingress(id, increment).await?;
	}
	Ok(())
}

#[cfg(all(feature = "cli", test))]
pub mod tests {

	#[tokio::test]
	pub async fn create_and_update_ingress() -> crate::LauncherResult<()> {
		use std::time::Duration;
		use crate::{api::{ingress::{init_ingress, send_ingress}, proxy::proxy_cli::ProxyCli}, initialize_core, store::ingress::IngressType};

		initialize_core(ProxyCli::new()).await?;

		let id = init_ingress(IngressType::Download { file_name: "Some-Mod-1.8.9.jar".into() }, "This is a test message", 100.0).await?;
		tokio::time::sleep(Duration::from_millis(2350)).await;

		send_ingress(&id, 10.0).await?;
		tokio::time::sleep(Duration::from_millis(2500)).await;

		send_ingress(&id, 20.0).await?;
		tokio::time::sleep(Duration::from_millis(2500)).await;

		send_ingress(&id, 70.0).await?;
		tokio::time::sleep(Duration::from_millis(2500)).await;

		drop(id);

		Ok(())
	}

}