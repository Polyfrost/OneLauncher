//! Downloads and probes one vendor's JRE, then records it in an in-memory store.
//!
//! Runs against `MemoryJavaStore`, so it needs no database and no ephemeral
//! `LauncherState` just to list packages.

use std::env;

use oneclient_common::paths::java_dir;
use oneclient_events::EventBus;
use oneclient_java::vendors::{
    AdoptiumRuntimeProvider, CorrettoRuntimeProvider, JavaRuntimeProvider, LibericaRuntimeProvider,
    ZuluRuntimeProvider,
};
use oneclient_java::{JavaResult, JavaService, MemoryJavaStore, check_java_runtime};
use oneclient_net::{NetConfig, RequestClient};

#[tokio::main]
async fn main() -> JavaResult<()> {
    let mut args = env::args().skip(1);
    let vendor = args.next().unwrap_or_else(|| {
        eprintln!("usage: install_provider <zulu|adoptium|corretto|liberica> [major]");
        std::process::exit(1);
    });

    let major: u32 = args
        .next()
        .unwrap_or_else(|| "21".to_string())
        .parse()
        .expect("major version must be a number");

    let net = RequestClient::new(NetConfig::default())?;
    // Nothing drains this, so events are dropped; the install path only uses it
    // to report progress.
    let (events, _rx) = EventBus::channel();

    let provider: Box<dyn JavaRuntimeProvider> = match vendor.as_str() {
        "zulu" => Box::new(ZuluRuntimeProvider),
        "adoptium" => Box::new(AdoptiumRuntimeProvider),
        "corretto" => Box::new(CorrettoRuntimeProvider),
        "liberica" => Box::new(LibericaRuntimeProvider),
        other => {
            eprintln!("unknown vendor '{other}', use zulu, adoptium, corretto, or liberica");
            std::process::exit(1);
        }
    };

    let packages = provider.list_packages(Some(major), &net).await?;
    let package = packages
        .into_iter()
        .find(|p| p.java_version.contains(&major))
        .unwrap_or_else(|| panic!("no {vendor} package for Java {major}"));

    println!("Installing package {:#?} to '{:?}'", package, java_dir()?);

    let executable = provider
        .install_package(&package, &net, &events, None)
        .await?;
    println!("Installed: {}", executable.display());

    let info = check_java_runtime(executable.display().to_string()).await?;
    println!("  version: {}", info.version);
    println!("  vendor:  {}", info.vendor);
    println!("  arch:    {}", info.os_arch);

    // Prove the service records it without a database in sight.
    let service = JavaService::new(MemoryJavaStore::new(), net, events);
    service.add_custom_runtime(executable).await?;
    println!("Registered runtimes: {:?}", service.list_runtimes().await?);

    Ok(())
}
