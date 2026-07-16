use oneclient_core::migration::lunar_client;

#[tokio::main]
async fn main() {
    match lunar_client::detect().await {
        Ok(Some(detection)) => {
            println!(
                "source: {} ({})",
                detection.source.display_name(),
                detection.source.id()
            );
            println!("root: {}", detection.root.display());
            println!("instances: {}", detection.instances.len());
            for i in &detection.instances {
                println!(
                    "  [{}] {} ({} {:?})  game_dir={}  categories={:?}",
                    i.instance_id,
                    i.folder_name,
                    i.mc_version,
                    i.mc_loader,
                    i.has_game_dir,
                    i.categories,
                );
            }
        }
        Ok(None) => println!("no lunar client install detected"),
        Err(err) => eprintln!("detection error: {err}"),
    }
}
