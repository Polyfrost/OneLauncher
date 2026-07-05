
use oneclient_core::dev;
use oneclient_core::packages::domain::GameLoader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let state = dev::ephemeral_state().await?;

    state.bundles.sync(&state.services).await?;

    let bundles = state
        .bundles
        .list_for(&state.services, "1.21.11", GameLoader::Fabric)
        .await?;

    println!("Visible Fabric bundles for 1.21.11: {}", bundles.len());
    for bundle in bundles {
        println!(
            "- {} ({}) at {}",
            bundle.name,
            bundle.version_id,
            bundle.path.display()
        );
    }

    let all = oneclient_db::dao::bundle::list_all(&state.services.db).await?;
    let hidden = all.iter().filter(|row| row.hidden != 0).count();
    println!("Total catalog rows: {} (hidden: {hidden})", all.len());

    Ok(())
}
