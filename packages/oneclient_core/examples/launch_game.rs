#![recursion_limit = "256"]

use oneclient_core::LauncherResult;
use oneclient_core::auth;
use oneclient_core::clusters::{ClusterManager, CreateClusterOptions};
use oneclient_core::dev;
use oneclient_core::packages::domain::GameLoader;

#[tokio::main]
async fn main() -> LauncherResult<()> {
    dev::initialize().await?;
    let state = dev::ephemeral_state().await?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mc_version = args.first().map(String::as_str).unwrap_or("1.21.4");
    let loader = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(GameLoader::Vanilla);

    let account = auth::add_offline_account("Example".to_string()).await?;
    println!("Using offline account {} ({})", account.username, account.id);

    let cluster = ClusterManager::create(
        &state,
        CreateClusterOptions::new(format!("launch-{mc_version}"), mc_version, loader),
    )
    .await?;

    println!("Launching {} ({mc_version} {loader:?})...", cluster.name);
    let game = oneclient_core::launch_cluster(&state, cluster.id, &account, true).await?;
    println!(
        "Launched cluster #{} (pid {:?}). Waiting 10s before exit...",
        game.cluster_id, game.pid
    );

    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    Ok(())
}
