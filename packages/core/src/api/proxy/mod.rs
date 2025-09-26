pub mod event;
mod r#impl;
pub mod message;

pub use r#impl::*;

#[async_trait::async_trait]
pub trait LauncherProxy: Send + Sync + std::fmt::Debug {
	async fn send_event(&self, event: event::LauncherEvent) -> crate::LauncherResult<()>;

	#[cfg(feature = "gui")]
	fn hide_main_window(&self) -> crate::LauncherResult<()>;

	#[cfg(feature = "gui")]
	fn show_main_window(&self) -> crate::LauncherResult<()>;
}

#[cfg(feature = "cli")]
pub type ProxyDynamic = ProxyCli;
#[cfg(not(feature = "cli"))]
pub type ProxyDynamic = ProxyEmpty;
