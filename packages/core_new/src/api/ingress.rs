use crate::{store::{ingress::{IngressId, IngressType}, State}, LauncherResult};

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

pub async fn send_ingress(id: IngressId) -> LauncherResult<()> {
	let state = State::get().await?;
	let processor = &state.ingress_processor;

	processor.send(id.0).await?;
	Ok(())
}

pub async fn update_ingress(id: IngressId, current: f64) -> LauncherResult<()> {
	let state = State::get().await?;
	let processor = &state.ingress_processor;

	processor.update(id.0, current).await
}

pub async fn finish_ingress(id: IngressId) -> LauncherResult<()> {
	let state = State::get().await?;
	let processor = &state.ingress_processor;

	processor.finish(id.0).await?;
	Ok(())
}