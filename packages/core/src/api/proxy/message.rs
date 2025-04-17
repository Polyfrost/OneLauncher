use serde::Serialize;

use super::event::send_event;

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
	Warn,
	Error,
}

pub fn send_message(level: MessageLevel, message: String) {
	tokio::spawn(async move {
		let payload = MessagePayload {
			level,
			message,
		};

		send_event(super::event::LauncherEvent::Message(payload)).await;
	});
}

#[macro_export]
macro_rules! send_message {
	($level:tt, $($message:tt)*) => {
		$crate::api::proxy::message::send_message(
			$level,
			format!($($message)*),
		)
	};
}

#[macro_export]
macro_rules! send_info {
	($($message:tt)*) => {
		$crate::api::proxy::message::send_message(
			$crate::api::proxy::message::MessageLevel::Info,
			format!($($message)*),
		)
	};
}

#[macro_export]
macro_rules! send_warning {
	($($message:tt)*) => {
		$crate::api::proxy::message::send_message(
			$crate::api::proxy::message::MessageLevel::Warn,
			format!($($message)*),
		)
	};
}

#[macro_export]
macro_rules! send_error {
	($($message:tt)*) => {
		$crate::api::proxy::message::send_message(
			$crate::api::proxy::message::MessageLevel::Error,
			format!($($message)*),
		)
	};
}
