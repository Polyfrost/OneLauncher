use crate::{store::ingress::IngressPayload, LauncherResult};

use super::LauncherProxy;

#[derive(Default)]
pub struct ProxyEmpty;

impl ProxyEmpty {
	#[must_use]
	pub const fn new() -> Self {
		Self
	}
}

#[async_trait::async_trait]
impl LauncherProxy for ProxyEmpty {

	async fn send_ingress(&self, ingress: IngressPayload) -> LauncherResult<()> {
		println!(
			"{} [{}] {}",
			ingress.percent.map_or("100%".to_string(), |p| format!("{:.2}%", p * 100.0)),
			ingress.id,
			ingress.message
		);
		Ok(())
	}

	async fn send_message(&self, message: crate::api::proxy::message::MessagePayload) -> LauncherResult<()> {
		println!("[{:?}] {}", message.level, message.message);
		Ok(())
	}

	#[cfg(feature = "gui")]
	fn hide_main_window(&self) -> crate::LauncherResult<()> {
		println!("hidden window");
		Ok(())
	}

	#[cfg(feature = "gui")]
	fn show_main_window(&self) -> crate::LauncherResult<()> {
		println!("shown window");
		Ok(())
	}

}
