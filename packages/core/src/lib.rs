use api::proxy::LauncherProxy;
use store::{proxy::ProxyState, Core, CoreOptions, Dirs, State};
use error::LauncherResult;
use logger::start_logger;

pub mod constants;
pub mod utils;
pub mod store;
pub mod api;
pub mod error;

mod logger;

pub use onelauncher_entity as entity;
pub use onelauncher_migration as migration;

pub async fn initialize_core(options: CoreOptions, proxy_backend: impl LauncherProxy + 'static) -> LauncherResult<()> {
	Core::initialize(options).await?;
	Dirs::get().await?;
	start_logger().await;

	ProxyState::initialize(proxy_backend).await?;
	State::get().await?;

	Ok(())
}
