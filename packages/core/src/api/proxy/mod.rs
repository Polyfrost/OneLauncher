pub mod message;

#[cfg(feature = "tauri")]
pub mod proxy_tauri;

#[cfg(feature = "cli")]
pub mod proxy_cli;

#[async_trait::async_trait]
pub trait LauncherProxy: Send + Sync {
	async fn send_ingress(&self, ingress: crate::store::ingress::IngressPayload) -> crate::LauncherResult<()>;
	async fn send_message(&self, message: message::MessagePayload) -> crate::LauncherResult<()>;

	#[cfg(feature = "gui")]
	fn hide_main_window(&self) -> crate::LauncherResult<()>;

	#[cfg(feature = "gui")]
	fn show_main_window(&self) -> crate::LauncherResult<()>;
}

