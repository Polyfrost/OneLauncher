use anyhow::anyhow;
use onelauncher_core::{api::{game::metadata::{download_minecraft_ingressed, download_version_info}, proxy::ProxyDynamic}, error::LauncherResult, initialize_core, store::{CoreOptions, State}};

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyDynamic::new()).await?;

	let state = State::get().await?;
	let mut metadata = state.metadata.write().await;
	let versions = metadata.get_vanilla_or_fetch().await?;

	let version = versions.versions.iter().find(|v| v.id == "1.20.4").ok_or_else(|| anyhow!("couldn't find 1.8.9 in version list"))?;
	let version_info = download_version_info(version, None, None, Some(true)).await?;

	download_minecraft_ingressed(&version_info, "x86", Some(true)).await?;

	Ok(())
}