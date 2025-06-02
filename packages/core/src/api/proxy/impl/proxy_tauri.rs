use tauri::{AppHandle, Manager};
use tracing::{error, warn};

use crate::{api::{proxy::{event::LauncherEvent, message::MessageLevel, LauncherProxy}, tauri::LauncherEventEmitter}, LauncherResult};

#[derive(Debug)]
pub struct ProxyTauri {
	emitter: LauncherEventEmitter<tauri::Wry>,
	handle: AppHandle,
}

impl ProxyTauri {
	#[must_use]
	pub fn new(handle: AppHandle) -> Self {
		Self {
			emitter: LauncherEventEmitter::new(handle.clone()),
			handle,
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

		Ok(self.emitter.send_event(event)?)
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