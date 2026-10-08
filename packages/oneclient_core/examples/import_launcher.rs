//! Detects Prism / MultiMC / Modrinth App installs and imports every instance
//! into a throwaway launcher dir under `target/`
//!
//! `cargo run -p oneclient_core --example import_launcher [launcher folder]`
//! (`PRISM_DIR` / `MODRINTH_APP_DIR` point detection at a test install)

use oneclient_content::packages::PackageStore;
use oneclient_core::dev;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let detections = match std::env::args().nth(1) {
        Some(folder) => oneclient_core::detect_external_folder(folder.into())
            .await?
            .into_iter()
            .collect(),
        None => oneclient_core::detect_external_launchers().await,
    };
    if detections.is_empty() {
        println!("no third-party launcher detected");
        return Ok(());
    }

    let state = dev::ephemeral_state().await?;
    let content = state.services.content();

    for detection in detections {
        println!(
            "{} at {} ({} instances)",
            detection.launcher.display_name(),
            detection.root.display(),
            detection.instances.len()
        );

        for instance in &detection.instances {
            println!(
                "  - {} ({} {} {:?}) groups={:?} pack={:?}",
                instance.name,
                instance.mc_version,
                instance.loader,
                instance.loader_version,
                instance.groups,
                instance.linked_pack,
            );

            let report =
                match oneclient_core::import_external_instance(&state, instance, None).await {
                    Ok(report) => report,
                    Err(err) => {
                        println!("    import failed: {err}");
                        continue;
                    }
                };
            let cluster = state.clusters.get(report.cluster_id).await?;
            let global = state.settings.read().global_game_settings.clone();
            let profile = state.clusters.resolve_settings(&global, &cluster).await?;
            println!(
                "    -> cluster #{} '{}' kind={:?} loader={:?} tags={:?} cover={:?} played={:?}",
                cluster.id,
                cluster.name,
                cluster.kind,
                cluster.mc_loader_version,
                cluster.tags,
                cluster.cover_path,
                cluster.overall_played,
            );
            println!(
                "       linked pack={} imported={} failed={:?} mods skipped={} blocked={}",
                report.linked_pack,
                report.content_imported,
                report.content_failed,
                report.mods_skipped,
                report.blocked.len(),
            );
            println!(
                "       mem={:?} jvm={:?} env={:?} res={:?} fullscreen={:?} pre={:?} wrapper={:?}",
                profile.mem_max,
                profile.launch_args,
                profile.launch_env,
                profile.resolution,
                profile.force_fullscreen,
                profile.hook_pre,
                profile.hook_wrapper,
            );

            let linked = PackageStore::list_linked_artifacts(cluster.id, &content).await?;
            println!("       {} linked files:", linked.len());
            for artifact in linked.iter().take(12) {
                println!(
                    "         {:?} {} enabled={} provider={:?} name={:?}",
                    artifact.content_type,
                    artifact.cluster_file_name,
                    artifact.enabled,
                    artifact.provider,
                    artifact.display_name,
                );
            }
            if linked.len() > 12 {
                println!("         ... and {} more", linked.len() - 12);
            }

            let game_dir = cluster.game_dir()?;
            println!("       game dir {}:", game_dir.display());
            let mut entries = tokio::fs::read_dir(&game_dir).await?;
            let mut names = Vec::new();
            while let Some(entry) = entries.next_entry().await? {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
            names.sort();
            println!("         {}", names.join(", "));
        }
    }

    Ok(())
}
