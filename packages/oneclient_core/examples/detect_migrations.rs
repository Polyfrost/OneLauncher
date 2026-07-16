#[tokio::main]
async fn main() {
    match oneclient_core::detect_migrations().await {
        Ok(detections) if detections.is_empty() => {
            println!("no migratable launcher detected");
        }
        Ok(detections) => {
            println!("sources: {}", detections.len());
            for detection in &detections {
                println!(
                    "\n{} ({})  root={}",
                    detection.source.display_name(),
                    detection.source.id(),
                    detection.root.display()
                );
                for i in &detection.instances {
                    println!(
                        "  {} ({} {:?})  game_dir={}  categories={:?}",
                        i.folder_name, i.mc_version, i.mc_loader, i.has_game_dir, i.categories,
                    );
                }
            }
        }
        Err(err) => eprintln!("detection error: {err}"),
    }
}
