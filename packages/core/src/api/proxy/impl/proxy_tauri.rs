use tauri::{AppHandle, Emitter, Manager};
use tauri_specta::Event;
use tracing::{error, warn};

use crate::{api::proxy::{event::LauncherEvent, LauncherProxy, message::MessageLevel}, LauncherResult};

#[derive(Debug)]
pub struct ProxyTauri {
	handle: AppHandle,
}

impl ProxyTauri {
	#[must_use]
	pub const fn new(handle: AppHandle) -> Self {
		Self {
			handle
		}
	}
}

#[async_trait::async_trait]
impl LauncherProxy for ProxyTauri {
	async fn send_event(&self, event: LauncherEvent) -> LauncherResult<()> {
		if let LauncherEvent::Message(message) = &event {
			match message.level {
				MessageLevel::Info => {},
				MessageLevel::Warn => warn!("{}", message.message),
				MessageLevel::Error => error!("{}", message.message),
			}
		}

		self.handle
			.emit(LauncherEvent::NAME, event)
			.map_err(Into::into)
	}

	#[tracing::instrument]
	fn hide_main_window(&self) -> LauncherResult<()> {
		if let Some(window) = self.handle.get_webview_window("main") {
			Ok(window.hide()?)
		} else {
			warn!("main window not found");
			Ok(())
		}
	}

	#[tracing::instrument]
	fn show_main_window(&self) -> LauncherResult<()> {
		if let Some(window) = self.handle.get_webview_window("main") {
			Ok(window.show()?)
		} else {
			warn!("main window not found");
			Ok(())
		}
	}

}