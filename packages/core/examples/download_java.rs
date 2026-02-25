use onelauncher_core::api::java;
use onelauncher_core::api::proxy::ProxyDynamic;
use onelauncher_core::error::LauncherResult;
use onelauncher_core::initialize_core;
use onelauncher_core::store::{CoreOptions, State};

#[tokio::main]
async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyDynamic::new()).await?;

	let _ = State::get().await?;

	let packages = java::get_zulu_packages().await?;

	let Some(pkg) = packages.first() else {
		return Err(anyhow::anyhow!("no zulu package found").into());
	};

	println!("found package: {pkg:#?}");

	let path = java::install_java_package(pkg).await?;

	println!("java package installed at '{}'", path.display());

	Ok(())
}
