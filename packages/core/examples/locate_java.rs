use onelauncher_core::{api::{java, proxy::proxy_empty::ProxyEmpty}, error::LauncherResult, initialize_core};

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(ProxyEmpty::new()).await?;

	let java = java::locate_java().await?;

	println!("Found {} java installations", java.len());
	for (path, info) in java {
		println!("{:?} = {}", info, path.display());
	}

	Ok(())
}