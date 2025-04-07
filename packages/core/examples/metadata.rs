use onelauncher_core::{api::proxy::ProxyDynamic, error::LauncherResult, initialize_core, store::{CoreOptions, State}};
use onelauncher_entity::loader::GameLoader;

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyDynamic::default()).await?;

	let state = State::get().await?;
	let metadata = &mut state.metadata.write().await;

	println!("Initialized: {}", metadata.initialized());
	println!("Minecraft: {:?}", metadata.get_vanilla(GameLoader::Vanilla).await.is_ok());
	println!("Forge: {:?}", metadata.get_modded(GameLoader::Forge).await.is_ok());
	println!("Initialized: {}", metadata.initialized());
	println!("NeoForge: {:?}", metadata.get_modded(GameLoader::NeoForge).await.is_ok());
	println!("Fabric: {:?}", metadata.get_modded(GameLoader::Fabric).await.is_ok());
	println!("Quilt: {:?}", metadata.get_modded(GameLoader::Quilt).await.is_ok());

	Ok(())
}