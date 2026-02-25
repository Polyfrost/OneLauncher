use onelauncher_core::api::java;
use onelauncher_core::api::proxy::ProxyEmpty;
use onelauncher_core::error::LauncherResult;
use onelauncher_core::initialize_core;
use onelauncher_core::store::CoreOptions;

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyEmpty::new()).await?;

	let java = java::locate_java().await?;

	println!("Found {} java installations", java.len());
	for (path, info) in java {
		println!("{:?} = {}", info, path.display());
	}

	Ok(())
}
