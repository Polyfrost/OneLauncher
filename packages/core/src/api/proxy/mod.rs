pub mod message;

#[cfg(feature = "tauri")] mod proxy_tauri;
#[cfg(feature = "cli")] mod proxy_cli;
mod proxy_empty;

#[cfg(feature = "tauri")] pub use proxy_tauri::ProxyTauri;
#[cfg(feature = "cli")] pub use proxy_cli::ProxyCli;
pub use proxy_empty::ProxyEmpty;

#[cfg(feature = "cli")] pub type ProxyDynamic = ProxyCli;
#[cfg(not(feature = "cli"))] pub type ProxyDynamic = ProxyEmpty;

#[async_trait::async_trait]
pub trait LauncherProxy: Send + Sync {
	async fn send_ingress(&self, ingress: crate::store::ingress::IngressPayload) -> crate::LauncherResult<()>;
	async fn send_message(&self, message: message::MessagePayload) -> crate::LauncherResult<()>;

	#[cfg(feature = "gui")]
	fn hide_main_window(&self) -> crate::LauncherResult<()>;

	#[cfg(feature = "gui")]
	fn show_main_window(&self) -> crate::LauncherResult<()>;
}

