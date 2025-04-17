use crate::{api::proxy::{event::LauncherEvent, LauncherProxy}, LauncherResult};

#[derive(Default, Debug)]
pub struct ProxyEmpty;

impl ProxyEmpty {
	#[must_use]
	pub const fn new() -> Self {
		Self
	}
}

#[async_trait::async_trait]
impl LauncherProxy for ProxyEmpty {
	async fn send_event(&self, event: LauncherEvent) -> LauncherResult<()> {
		match event {
			LauncherEvent::Ingress(ingress) => {
				println!(
					"{} [{}] {}",
					ingress.percent.map_or("100%".to_string(), |p| format!("{:.2}%", p * 100.0)),
					ingress.id,
					ingress.message
				);
			},
			LauncherEvent::Message(message) => {
				println!("[{:?}] {}", message.level, message.message);
			},
			LauncherEvent::Process(payload) => {
				println!("{payload:#?}");
			},
		}

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
