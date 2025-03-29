pub mod constants;
pub mod utils;
pub mod store;
pub mod api;

mod error;
mod logger;

use api::proxy::LauncherProxy;
pub use error::{LauncherResult, LauncherError};
pub use logger::start_logger;
use store::{proxy::ProxyState, Dirs, State};

pub async fn initialize_core(proxy_backend: impl LauncherProxy + 'static) -> LauncherResult<()> {
	Dirs::get().await?;
	start_logger().await;

	ProxyState::initialize(proxy_backend).await?;
	State::get().await?;

	Ok(())
}