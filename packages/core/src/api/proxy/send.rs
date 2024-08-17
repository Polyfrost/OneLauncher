use super::IngressId;
use crate::api::proxy::{
	ClusterPayloadType, Ingress, IngressType, InternetPayload, ProcessPayloadType, ProxyError,
};
use crate::proxy::OfflinePayload;
use crate::store::{ClusterPath, IngressProcessType, IngressProcessor};
use tauri::Emitter;
use uuid::Uuid;

#[cfg(feature = "cli")]
use crate::constants::CLI_TOTAL_INGRESS;

#[cfg(feature = "tauri")]
use tauri_specta::Event;

#[cfg(feature = "tauri")]
use crate::api::proxy::{ClusterPayload, IngressPayload, MessagePayload, ProcessPayload};

#[onelauncher_macros::memory]
pub async fn init_ingress(
	ingress_type: IngressType,
	total: f64,
	title: &str,
) -> crate::Result<IngressId> {
	let key = init_ingress_internal(ingress_type, total, title).await?;
	IngressProcessor::add_ingress(IngressProcessType::IngressFeed, key.0).await?;
	Ok(key)
}

#[onelauncher_macros::memory]
pub async fn init_ingress_internal(
	ingress_type: IngressType,
	total: f64,
	title: &str,
) -> crate::Result<IngressId> {
	let proxy_state = crate::ProxyState::get().await?;
	let key = IngressId(Uuid::new_v4());

	proxy_state.ingress_feeds.write().await.insert(
		key.0,
		Ingress {
			ingress_uuid: key.0,
			message: title.to_string(),
			total,
			current: 0.0,
			last_sent: 0.0,
			ingress_type,
			#[cfg(feature = "cli")]
			cli: {
				let pb = indicatif::ProgressBar::new(CLI_TOTAL_INGRESS);
				pb.set_position(0);
				pb.set_style(indicatif::ProgressStyle::default_bar().template(
                "{spinner:.green}, [{elapsed_precise}] [{bar:.lime/green}] {pos}/{len} {msg}"
            ).unwrap().progress_chars("#>-"));
				pb
			},
		},
	);

	send_ingress(&key, 0.0, None).await?;
	Ok(key)
}

pub async fn init_or_edit_ingress(
	id: Option<IngressId>,
	ingress_type: IngressType,
	total: f64,
	title: &str,
) -> crate::Result<IngressId> {
	if let Some(id) = id {
		edit_ingress(&id, ingress_type, total, title).await?;

		Ok(id)
	} else {
		init_ingress(ingress_type, total, title).await
	}
}

pub async fn edit_ingress(
	id: &IngressId,
	ingress_type: IngressType,
	total: f64,
	title: &str,
) -> crate::Result<()> {
	let proxy_state = crate::ProxyState::get().await?;

	if let Some(ingress) = proxy_state.ingress_feeds.write().await.get_mut(&id.0) {
		ingress.ingress_type = ingress_type;
		ingress.total = total;
		ingress.message = title.to_string();
		ingress.current = 0.0;
		ingress.last_sent = 0.0;
		#[cfg(feature = "cli")]
		{
			ingress.cli.reset();
		}
	};

	send_ingress(id, 0.0, None).await?;
	Ok(())
}

#[allow(unused_variables)]
#[tracing::instrument(level = "debug")]
#[onelauncher_macros::memory]
pub async fn send_ingress(
	key: &IngressId,
	increment: f64,
	message: Option<&str>,
) -> crate::Result<()> {
	let proxy_state = crate::ProxyState::get().await?;
	let mut ingress = proxy_state.ingress_feeds.write().await;
	let ingress = match ingress.get_mut(&key.0) {
		Some(f) => f,
		None => {
			return Err(ProxyError::NoIngressFound(key.0).into());
		}
	};

	ingress.current += increment;
	let display = ingress.current / ingress.total;
	let display_conv = if display >= 1.0 { None } else { Some(display) };

	if f64::abs(display - ingress.last_sent) > 0.005 {
		#[cfg(feature = "cli")]
		{
			ingress.cli.set_message(
				message
					.map(|x| x.to_string())
					.unwrap_or(ingress.message.clone()),
			);
			ingress
				.cli
				.set_position((display * CLI_TOTAL_INGRESS as f64).round() as u64);
		}

		#[cfg(feature = "tauri")]
		use tauri::Emitter;

		#[cfg(feature = "tauri")]
		proxy_state
			.app
			.emit(
				IngressPayload::NAME,
				IngressPayload {
					fraction: display_conv,
					message: message.unwrap_or(&ingress.message).to_string(),
					event: ingress.ingress_type.clone(),
					ingress_uuid: ingress.ingress_uuid,
				},
			)
			.map_err(ProxyError::from)?;

		ingress.last_sent = display;
	}

	Ok(())
}

pub async fn send_message(message: &str) -> crate::Result<()> {
	#[cfg(feature = "tauri")]
	{
		let proxy_state = crate::ProxyState::get().await?;
		proxy_state
			.app
			.emit(
				MessagePayload::NAME,
				MessagePayload {
					message: message.to_string(),
				},
			)
			.map_err(ProxyError::from)?;
	}

	tracing::warn!("{}", message);
	Ok(())
}

pub async fn send_offline(offline: bool) -> crate::Result<()> {
	#[cfg(feature = "tauri")]
	{
		let proxy_state = crate::ProxyState::get().await?;
		proxy_state
			.app
			.emit(
				OfflinePayload::NAME,
				OfflinePayload {
					offline,
				},
			)
			.map_err(ProxyError::from)?;
	}

	Ok(())
}

pub async fn send_internet(internet: InternetPayload) -> crate::Result<()> {
	tracing::debug!("operation {}", serde_json::to_string(&internet)?);
	#[cfg(feature = "tauri")]
	{
		let proxy_state = crate::ProxyState::get().await?;
		proxy_state
			.app
			.emit(
				InternetPayload::NAME,
				internet
			)
			.map_err(ProxyError::from)?;
	}

	Ok(())
}

pub async fn send_process(
	uuid: Uuid,
	pid: u32,
	event: ProcessPayloadType,
	message: &str,
) -> crate::Result<()> {
	#[cfg(feature = "tauri")]
	{
		let proxy_state = crate::ProxyState::get().await?;
		proxy_state
			.app
			.emit(
				ProcessPayload::NAME,
				ProcessPayload {
					uuid,
					pid,
					event,
					message: message.to_string(),
				},
			)
			.map_err(ProxyError::from)?;
	}

	Ok(())
}

pub async fn send_cluster(
	uuid: Uuid,
	cluster_path: &ClusterPath,
	name: &str,
	event: ClusterPayloadType,
) -> crate::Result<()> {
	#[cfg(feature = "tauri")]
	{
		let path = cluster_path.full_path().await?;
		let proxy_state = crate::ProxyState::get().await?;
		proxy_state
			.app
			.emit(
				ClusterPayload::NAME,
				ClusterPayload {
					uuid,
					cluster_path: cluster_path.clone(),
					path,
					name: name.to_string(),
					event,
				},
			)
			.map_err(ProxyError::from)?;
	}

	Ok(())
}
