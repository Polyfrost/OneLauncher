use anyhow::anyhow;
use onelauncher_core::api::game::metadata::{download_minecraft_ingressed, download_version_info};
use onelauncher_core::api::proxy::ProxyDynamic;
use onelauncher_core::error::LauncherResult;
use onelauncher_core::initialize_core;
use onelauncher_core::store::{CoreOptions, State};

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyDynamic::new()).await?;

	let state = State::get().await?;
	let mut metadata = state.metadata.write().await;
	let versions = metadata.get_vanilla_or_fetch().await?;

	let version = versions
		.versions
		.iter()
		.find(|v| v.id == "1.20.4")
		.ok_or_else(|| anyhow!("couldn't find 1.20.4 in version list"))?;
	let version_info = download_version_info(version, None, None, Some(true)).await?;

	let minecraft_updated = true;

	download_minecraft_ingressed(&version_info, "x86", minecraft_updated, Some(true)).await?;

	Ok(())
}
