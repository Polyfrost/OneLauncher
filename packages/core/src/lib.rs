#![feature(let_chains)]

use api::proxy::LauncherProxy;
use store::{proxy::ProxyState, semaphore::SemaphoreStore, Core, CoreOptions, Dirs, State};
use error::LauncherResult;
use logger::start_logger;

pub mod api;
pub mod constants;
pub mod error;
pub mod store;
pub mod utils;

mod logger;

pub use onelauncher_macro as macros;
pub use onelauncher_entity as entity;
pub use onelauncher_migration as migration;

pub async fn initialize_core(options: CoreOptions, proxy_backend: impl LauncherProxy + 'static) -> LauncherResult<()> {
	Core::initialize(options).await?;
	Dirs::get().await?;
	start_logger().await;

	SemaphoreStore::get().await;
	ProxyState::initialize(proxy_backend).await?;
	let _ = State::get().await?;

	tracing::info!("Core initialized successfully");

	Ok(())
}
