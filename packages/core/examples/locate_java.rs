use onelauncher_core::{api::{java, proxy::ProxyEmpty}, error::LauncherResult, initialize_core, store::CoreOptions};

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