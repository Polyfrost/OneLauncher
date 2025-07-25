#![feature(slice_as_array)]
#![allow(clippy::struct_excessive_bools)]

use api::proxy::LauncherProxy;
use error::LauncherResult;
pub use logger::start_logger;
use store::proxy::ProxyState;
use store::semaphore::SemaphoreStore;
use store::{Core, CoreOptions, Dirs, State};

pub mod api;
pub mod constants;
pub mod error;
pub mod store;
pub mod utils;

mod logger;

pub use onelauncher_macro::*;
pub use {onelauncher_entity as entity, onelauncher_migration as migration};

pub async fn initialize_core(
	options: CoreOptions,
	proxy_backend: impl LauncherProxy + 'static,
) -> LauncherResult<()> {
	Core::initialize(options).await?;
	Dirs::get().await?;
	start_logger().await;

	SemaphoreStore::get().await;
	ProxyState::initialize(proxy_backend).await?;
	let _ = State::get().await?;

	tracing::info!("Core initialized successfully");

	Ok(())
}
