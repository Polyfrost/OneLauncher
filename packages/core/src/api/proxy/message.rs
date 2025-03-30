use serde::Serialize;

#[onelauncher_macro::specta(with_event)]
#[derive(Serialize, Debug, Clone)]
pub struct MessagePayload {
	pub level: MessageLevel,
	pub message: String,
}

#[onelauncher_macro::specta]
#[derive(Serialize, Debug, Clone)]
pub enum MessageLevel {
	Info,
	Warning,
	Error,
}

#[macro_export]
macro_rules! send_warning {
	($($message:tt)*) => {{
		tokio::spawn(async move {
			let _ = match $crate::store::proxy::ProxyState::get() {
				Ok(proxy) => {
					let payload = $crate::api::proxy::message::MessagePayload {
						level: $crate::api::proxy::message::MessageLevel::Warning,
						message: format!($($message)*),
					};
					let _ = proxy.send_message(payload).await;
				},
				Err(err) => {
					tracing::warn!($($message)*);
					tracing::warn!("failed to send warning: {}", err);
				},
			};
		})
	}};
}

#[macro_export]
macro_rules! send_error {
	($($message:tt)*) => {{
		tokio::spawn(async move {
			let _ = match $crate::store::proxy::ProxyState::get() {
				Ok(proxy) => {
					let payload = $crate::api::proxy::message::MessagePayload {
						level: $crate::api::proxy::message::MessageLevel::Error,
						message: format!($($message)*),
					};
					let _ = proxy.send_message(payload).await;
				},
				Err(err) => {
					tracing::error!($($message)*);
					tracing::error!("failed to send error: {}", err);
				},
			};
		})
	}};
}
