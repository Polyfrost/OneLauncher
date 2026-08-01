//! Lists the JDK packages each vendor currently publishes.
//!
//! Needs only a network client, no database or launcher state. That is the
//! point of `JavaStore` being a port: the vendor providers are exercisable on
//! their own.

use std::env;

use oneclient_java::vendors::{JavaVendor, runtime_providers};
use oneclient_java::{JavaPackage, JavaResult};
use oneclient_net::{NetConfig, RequestClient};

#[tokio::main]
async fn main() -> JavaResult<()> {
    let arg = env::args().nth(1).unwrap_or_else(|| "21".to_string());
    let major: Option<u32> = if arg == "all" {
        None
    } else {
        Some(arg.parse().expect("usage: list_providers [major|all]"))
    };

    let net = RequestClient::new(NetConfig::default())?;

    match major {
        Some(major) => println!("Listing JDK packages for Java {major} (metadata only):\n"),
        None => println!("Listing JDK packages for all majors (metadata only):\n"),
    }

    for provider in runtime_providers() {
        match provider.list_packages(major, &net).await {
            Ok(packages) => print_vendor(provider.vendor(), &packages),
            Err(err) => println!("{:?}: ERROR {err}", provider.vendor()),
        }
    }

    Ok(())
}

fn print_vendor(vendor: JavaVendor, packages: &[JavaPackage]) {
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
