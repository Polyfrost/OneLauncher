
use std::env;

use oneclient_core::LauncherResult;
use oneclient_core::dev;
use oneclient_core::java::vendors::{JavaVendor, runtime_providers};
use oneclient_core::logger;

#[tokio::main]
async fn main() -> LauncherResult<()> {
    logger::init_debug()?;

    let arg = env::args().nth(1).unwrap_or_else(|| "21".to_string());
    let major: Option<u32> = if arg == "all" {
        None
    } else {
        Some(arg.parse().expect("usage: java_list_providers [major|all]"))
    };

    let services = dev::ephemeral_services().await?;

    match major {
        Some(major) => println!("Listing JRE packages for Java {major} (metadata only):\n"),
        None => println!("Listing JRE packages for all majors (metadata only):\n"),
    }

    for provider in runtime_providers() {
        match provider.list_packages(major, &services).await {
            Ok(packages) => print_vendor(provider.vendor(), &packages),
            Err(err) => println!("{:?}: ERROR {err}", provider.vendor()),
        }
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
