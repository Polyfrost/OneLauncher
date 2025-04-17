use serde::Serialize;

use crate::{error::LauncherResult, store::proxy::ProxyState};

#[onelauncher_macro::specta(with_event)]
#[derive(Debug, Serialize)]
pub enum LauncherEvent {
	Ingress(crate::api::ingress::IngressPayload),
	Message(crate::api::proxy::message::MessagePayload),
	Process(crate::api::processes::ProcessPayload),
}

pub async fn send_event(event: LauncherEvent) {
	let Ok(proxy) = ProxyState::get() else {
		tracing::error!("failed to get proxy state");
		return;
	};

	if let Err(e) = proxy.send_event(event).await {
		tracing::error!("failed to send event: {e}");
	}
}

pub async fn try_send_event(event: LauncherEvent) -> LauncherResult<()> {
	let proxy = ProxyState::get()?;
	proxy.send_event(event).await
}
