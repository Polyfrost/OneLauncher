#![recursion_limit = "256"]
use oneclient_core::clusters::{ClusterManager, CreateClusterOptions};
use oneclient_core::dev;
use oneclient_core::packages::domain::GameLoader;
use oneclient_core::LauncherResult;

#[tokio::main]
async fn main() -> LauncherResult<()> {
    dev::initialize().await?;
    let state = dev::ephemeral_state().await?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mc_version = args.first().map(String::as_str).unwrap_or("26.1");
    let loader = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(GameLoader::Vanilla);
    let loader_version = args.get(2).map(String::as_str);

    let cluster = ClusterManager::create(
        &state,
        CreateClusterOptions {
            name: format!("download-{mc_version}"),
            mc_version: mc_version.to_string(),
            mc_loader: loader,
            mc_loader_version: loader_version.map(str::to_string),
            mem_max: None,
        },
    )
    .await?;

    println!(
        "Preparing cluster {} ({} {:?})...",
        cluster.name, cluster.mc_version, cluster.mc_loader
    );

    let ready = ClusterManager::prepare(&state, cluster.id, false, true, true, None).await?;
    println!("Cluster ready at {}", ready.dir()?.display());

    Ok(())
}
