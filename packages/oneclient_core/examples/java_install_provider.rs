
use std::env;

use oneclient_core::dev;
use oneclient_core::java::check_java_runtime;
use oneclient_core::java::vendors::AdoptiumRuntimeProvider;
use oneclient_core::java::vendors::JavaRuntimeProvider;
use oneclient_core::java::vendors::ZuluRuntimeProvider;
use oneclient_core::logger;
use oneclient_core::LauncherResult;
use oneclient_core::paths::java_dir;

#[tokio::main]
async fn main() -> LauncherResult<()> {
	logger::init_debug()?;

	let mut args = env::args().skip(1);
	let vendor = args
		.next()
		.unwrap_or_else(|| {
			eprintln!("usage: java_install_provider <zulu|adoptium|corretto> [major]");
			std::process::exit(1);
		});

	let major: u32 = args
		.next()
		.unwrap_or_else(|| "21".to_string())
		.parse()
		.expect("major version must be a number");

	let services = dev::ephemeral_services().await?;

    let provider: Box<dyn JavaRuntimeProvider> = match vendor.as_str() {
        "zulu" => Box::new(ZuluRuntimeProvider),
        "adoptium" => Box::new(AdoptiumRuntimeProvider),
        other => {
			eprintln!("unknown vendor '{other}', use zulu, adoptium, or corretto");
			std::process::exit(1);
		}
    };

    let packages = provider.list_packages_by_major(major, &services).await?;
    let package = packages
        .into_iter()
        .find(|p| p.java_version.contains(&major))
        .unwrap_or_else(|| panic!("no {vendor} package for Java {major}"));

    println!("Installing package {:#?} to '{:?}'", package, java_dir()?);

    let executable = provider.install_package(&package, &services).await?;

	println!("Installed: {}", executable.display());

	let info = check_java_runtime(executable.display().to_string()).await?;
	println!("  version: {}", info.version);
	println!("  vendor:  {}", info.vendor);
	println!("  arch:    {}", info.os_arch);

	Ok(())
}
