
use std::env;

use oneclient_core::LauncherResult;
use oneclient_core::dev;
use oneclient_core::java::vendors::{JavaVendor, runtime_providers};
use oneclient_core::logger;

#[tokio::main]
async fn main() -> LauncherResult<()> {
    logger::init_debug()?;

    let major: u32 = env::args()
        .nth(1)
        .unwrap_or_else(|| "21".to_string())
        .parse()
        .expect("usage: java_list_providers [major_version]");

    let services = dev::ephemeral_services().await?;

    println!("Listing JRE packages for Java {major} (metadata only):\n");

    for provider in runtime_providers() {
        let packages = provider.list_packages_by_major(major, &services).await?;
        print_vendor(provider.vendor(), &packages);
    }

    Ok(())
}

fn print_vendor(vendor: JavaVendor, packages: &[oneclient_core::java::JavaPackage]) {
    let label = format!("{vendor:?}");

    if packages.is_empty() {
        println!("{label}: (none)");
        return;
    }

    for package in packages {
        println!(
            "{label}: {} [{}] {}",
            package.name,
            package
                .java_version
                .first()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            package.download_url
        );
    }
}
