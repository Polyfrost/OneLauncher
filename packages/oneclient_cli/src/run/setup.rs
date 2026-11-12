use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, anyhow, bail};
use clap::{Args, Subcommand};
use oneclient_common::domain::ProviderId;
use oneclient_common::paths;
use oneclient_core::clusters::ModpackSource;
use oneclient_core::{
    Cluster, ExternalDetection, ExternalInstance, ImportChoices, ImportTarget, LauncherState,
    MigrationSource, SourceInstance,
};
use oneclient_events::{Choice, Prompt};
use serde::{Deserialize, Serialize};

use super::about::save_settings;
use super::{Host, clusters_changed};
use crate::Output;
use crate::output::{bold, dim, good, heading, table, warn};

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct SetupArgs {
    /// Where to keep game data, the default folder when left out
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Accept the terms of service without asking
    #[arg(long)]
    pub accept_terms: bool,
    /// Do not offer to import from another launcher
    #[arg(long)]
    pub skip_import: bool,
    /// Do not offer to sign in
    #[arg(long)]
    pub skip_login: bool,
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum ImportCommand {
    /// Look for game folders and instances in other launchers to bring over
    Detect {
        /// Also look in this launcher folder, for portable installs
        #[arg(long)]
        from: Option<PathBuf>,
    },
    /// Turn an instance from Prism, PolyMC, MultiMC or the Modrinth App into a cluster
    Instance {
        /// Instance name or id, see `import detect`
        instance: String,
        /// Launcher folder to read it from instead of the usual places
        #[arg(long)]
        from: Option<PathBuf>,
        /// Swap mods known to cause problems for the recommended replacements
        #[arg(long)]
        improve: bool,
        /// Folders to look in for files CurseForge makes you download by hand
        #[arg(long)]
        scan: Vec<PathBuf>,
    },
    /// Import a .mrpack, a launcher's instance export or a CurseForge modpack zip
    File {
        path: PathBuf,
        /// Swap mods known to cause problems for the recommended replacements
        #[arg(long)]
        improve: bool,
        #[arg(long)]
        scan: Vec<PathBuf>,
        /// Install even when the modpack is known to cause problems
        #[arg(long)]
        force: bool,
    },
    /// Copy an old OneClient's or the vanilla launcher's game folder into OneClient
    Run {
        /// Folder name, see `import detect`
        folder: String,
        /// Copy into the matching cluster's own folder instead of the shared one
        #[arg(long)]
        dedicated: bool,
    },
}

#[must_use]
pub fn is_set_up() -> bool {
    let settings = paths::settings_file().is_ok_and(|path| path.exists());
    let database = paths::database_file().is_ok_and(|path| path.exists());
    settings || database
}

pub async fn prepare_setup(args: &SetupArgs) -> anyhow::Result<()> {
    if is_set_up() {
        if args.data_dir.is_some() {
            bail!("OneClient is already set up, move its data with `oneclient_cli storage move`");
        }
        return Ok(());
    }
    oneclient_core::settings::data_dir::apply(args.data_dir.clone())
        .await
        .map_err(|reason| anyhow!(reason))
}

pub(super) async fn run(
    host: Host,
    state: &Arc<LauncherState>,
    args: SetupArgs,
    out: &Output,
) -> anyhow::Result<()> {
    if host == Host::Gui {
        bail!("OneClient is open, so it is already set up");
    }
    out.line(format!(
        "{} in {}",
        heading("Setting up OneClient"),
        paths::data_dir()?.display()
    ));

    terms(state, args.accept_terms, out).await?;

    out.line("Fetching versions and bundles");
    let content = state.services.content();
    if let Err(err) = state.versions.sync(&state.services).await {
        tracing::warn!("versions manifest sync failed: {err:#}");
    }
    if let Err(err) = state.bundles.sync(&content).await {
        tracing::warn!("bundle catalog sync failed: {err:#}");
    }
    oneclient_core::ensure_from_versions(state).await?;
    oneclient_core::ensure_from_bundles(state).await?;
    clusters_changed(state);

    if !args.skip_import
        && let Ok(Some(detection)) = oneclient_core::detect_migration().await
    {
        offer_import(state, detection.source, &detection.instances, out).await;
    }

    if !args.skip_import {
        let found: usize = oneclient_core::detect_external_launchers()
            .await
            .iter()
            .map(|detection| detection.instances.len())
            .sum();
        if found > 0 {
            out.line(format!(
                "Found {found} instances in other launchers, see `oneclient_cli import detect` to bring them over"
            ));
        }
    }

    if !args.skip_login && state.auth.list_accounts().await.is_empty() {
        let prompt = Prompt::new(
            "Sign in to Minecraft?",
            "OneClient needs a Microsoft account to play.",
        )
        .option(Choice::primary("setup.login", "Sign in"), ())
        .dismiss("Later");
        if let Ok(Some(_)) = state.services.events.ask(prompt).await {
            super::account::run(state, crate::AccountCommand::Login { offline: None }, out).await?;
        }
    }

    let clusters: Vec<Cluster> = state
        .clusters
        .list()
        .await?
        .into_iter()
        .filter(Cluster::uses_bundles)
        .collect();
    for cluster in &clusters {
        super::game::choose_bundles(state, cluster).await;
    }

    let mut versions: Vec<String> = clusters.iter().map(|c| c.mc_version.clone()).collect();
    versions.sort();
    versions.dedup();
    save_settings(state, |settings| {
        for version in versions {
            if !settings.seen_versions.contains(&version) {
                settings.seen_versions.push(version);
            }
        }
        settings.seen_onboarding = true;
    })
    .await?;

    out.line(format!(
        "{} OneClient is ready with {} clusters, play one with `oneclient_cli launch <cluster>`",
        good("✓"),
        state.clusters.list().await?.len()
    ));
    Ok(())
}

async fn terms(state: &LauncherState, accept: bool, out: &Output) -> anyhow::Result<()> {
    let document = oneclient_core::fetch_terms(&state.services.requester).await?;
    if state.settings.read().accepted_tos_version >= document.version {
        return Ok(());
    }

    let accepted = accept
        || {
            let prompt = Prompt::new(
            "Terms of service",
            format!(
                "Read them at {} or with `oneclient_cli terms show`. Declining turns off Polyfrost's online services.",
                document.terms_url()
            ),
        )
        .option(Choice::primary("terms.accept", "Accept"), true)
        .option(Choice::danger("terms.decline", "Decline"), false)
        .dismiss("Stop");
            match state.services.events.ask(prompt).await {
                Ok(Some(chosen)) => chosen.value,
                _ => bail!("setup needs an answer to the terms, or pass --accept-terms"),
            }
        };

    if accepted {
        save_settings(state, |settings| {
            settings.accepted_tos_version = document.version;
            settings.accepted_privacy_version = document.privacy_version.unwrap_or(0);
            settings.declined_tos = false;
        })
        .await?;
        out.line(format!("{} Accepted the terms", good("✓")));
    } else {
        oneclient_common::consent::decline();
        save_settings(state, |settings| {
            settings.declined_tos = true;
            settings.accepted_tos_version = 0;
            settings.accepted_privacy_version = 0;
        })
        .await?;
        out.line("Declined the terms, Polyfrost's online services are off");
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ImportChoice {
    Shared,
    Dedicated,
}

async fn offer_import(
    state: &Arc<LauncherState>,
    source: MigrationSource,
    instances: &[SourceInstance],
    out: &Output,
) {
    for instance in instances {
        let mut prompt = Prompt::new(
            format!(
                "Import {} from {}?",
                instance.folder_name,
                source.display_name()
            ),
            format!(
                "Minecraft {} with {}",
                instance.import_version(),
                instance.mc_loader
            ),
        )
        .option(
            Choice::primary("import.shared", "Import"),
            ImportChoice::Shared,
        );
        if source != MigrationSource::Vanilla {
            prompt = prompt.option(
                Choice::new("import.dedicated", "Import into its own folder"),
                ImportChoice::Dedicated,
            );
        }
        let Ok(Some(chosen)) = state.services.events.ask(prompt.dismiss("Skip")).await else {
            continue;
        };
        let dedicated = matches!(chosen.value, ImportChoice::Dedicated);
        if let Err(err) = import(state, source, instance, dedicated, out).await {
            out.error(format!(
                "Could not import {}: {err:#}",
                instance.folder_name
            ));
        }
    }
}

async fn import(
    state: &Arc<LauncherState>,
    source: MigrationSource,
    instance: &SourceInstance,
    dedicated: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let target = if dedicated {
        let clusters = state.clusters.list().await?;
        let cluster = clusters
            .iter()
            .find(|c| {
                c.mc_version == instance.import_version() && c.mc_loader == instance.mc_loader
            })
            .with_context(|| {
                format!(
                    "no cluster for Minecraft {} with {} to import into",
                    instance.import_version(),
                    instance.mc_loader
                )
            })?;
        ImportTarget::Dedicated {
            new_cluster_id: cluster.id,
        }
    } else {
        ImportTarget::Shared
    };

    out.line(format!(
        "Importing {} from {}",
        bold(&instance.folder_name),
        source.display_name()
    ));
    oneclient_core::import_migration_game_dir(state, source, &instance.folder_name, target).await?;
    clusters_changed(state);
    out.line(format!("{} Imported {}", good("✓"), instance.folder_name));
    Ok(())
}

async fn detections(from: Option<PathBuf>) -> anyhow::Result<Vec<ExternalDetection>> {
    let mut found = oneclient_core::detect_external_launchers().await;
    if let Some(root) = from {
        let root = dunce::canonicalize(&root).unwrap_or(root);
        let extra = oneclient_core::detect_external_folder(root.clone())
            .await?
            .with_context(|| {
                format!(
                    "no Prism, MultiMC or Modrinth App install in {}",
                    root.display()
                )
            })?;
        if !found
            .iter()
            .any(|detection| oneclient_core::same_external_folder(&detection.root, &extra.root))
        {
            found.push(extra);
        }
    }
    Ok(found)
}

pub(super) async fn import_command(
    state: &Arc<LauncherState>,
    command: ImportCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        ImportCommand::Detect { from } => {
            let mut any = false;
            if let Ok(Some(detection)) = oneclient_core::detect_migration().await {
                any = true;
                out.line(format!(
                    "{} {}",
                    heading(detection.source.display_name()),
                    dim(detection.root.display())
                ));
                let rows: Vec<Vec<String>> = detection
                    .instances
                    .iter()
                    .map(|instance| {
                        vec![
                            bold(&instance.folder_name),
                            instance.import_version().to_string(),
                            instance.mc_loader.to_string(),
                        ]
                    })
                    .collect();
                out.line(table(&["FOLDER", "MINECRAFT", "LOADER"], &rows));
                out.line(dim(
                    "Bring one over with `oneclient_cli import run <folder>`\n",
                ));
            }
            for detection in detections(from).await? {
                any = true;
                out.line(format!(
                    "{} {}",
                    heading(detection.launcher.display_name()),
                    dim(detection.root.display())
                ));
                if let Some(problem) = &detection.problem {
                    out.error(warn(format!("Couldn't read this install: {problem}")));
                }
                let rows: Vec<Vec<String>> = detection
                    .instances
                    .iter()
                    .map(|instance| {
                        vec![
                            bold(&instance.name),
                            instance.mc_version.clone(),
                            match &instance.unsupported_loader {
                                Some(loader) => warn(format!("{loader}, not supported")),
                                None => instance.loader.to_string(),
                            },
                            dim(&instance.id),
                        ]
                    })
                    .collect();
                out.line(table(&["INSTANCE", "MINECRAFT", "LOADER", "ID"], &rows));
                out.line(dim(
                    "Bring one over with `oneclient_cli import instance <name>`\n",
                ));
            }
            if !any {
                out.line("No other launcher's game folders or instances were found");
            }
            Ok(())
        }
        ImportCommand::Instance {
            instance,
            from,
            improve,
            scan,
        } => {
            let lower = instance.to_lowercase();
            let found: Vec<ExternalInstance> = detections(from)
                .await?
                .into_iter()
                .flat_map(|detection| detection.instances)
                .filter(|candidate| {
                    candidate.id == instance || candidate.name.to_lowercase() == lower
                })
                .collect();
            let picked = match found.as_slice() {
                [only] => only.clone(),
                [] => bail!("no instance named \"{instance}\", see `oneclient_cli import detect`"),
                many => bail!(
                    "\"{instance}\" matches {} instances, use its id: {}",
                    many.len(),
                    many.iter()
                        .map(|i| i.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            import_external(state, vec![picked], false, improve, scan, out).await
        }
        ImportCommand::File {
            path,
            improve,
            scan,
            force,
        } => {
            let path = dunce::canonicalize(&path).unwrap_or(path);
            let is_mrpack = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case(oneclient_content::modpacks::SHARE_EXTENSION)
                });
            if !is_mrpack && oneclient_core::is_instance_archive(&path).await {
                let archive = oneclient_core::open_instance_archive(&path).await?;
                let outcome =
                    import_external(state, archive.instances.clone(), true, improve, scan, out)
                        .await;
                oneclient_core::discard_instance_archive(&archive.workdir).await;
                return outcome;
            }
            if !is_mrpack {
                oneclient_content::modpacks::detect_format(&path).await.map_err(|_| {
                    anyhow!("this file isn't a OneClient, Prism Launcher, MultiMC or CurseForge export")
                })?;
            }
            super::modpack::install(state, ModpackSource::File(path), None, scan, force, out)
                .await
                .map(|_| ())
        }
        ImportCommand::Run { folder, dedicated } => {
            let Some(detection) = oneclient_core::detect_migration().await? else {
                out.line("No old OneClient or vanilla launcher game folders were found");
                return Ok(());
            };
            let instance = detection
                .instances
                .iter()
                .find(|instance| instance.folder_name == folder)
                .with_context(|| format!("no folder named {folder}, see `import detect`"))?;
            if dedicated && detection.source == MigrationSource::Vanilla {
                bail!("a vanilla game folder can only go into the shared folder");
            }
            import(state, detection.source, instance, dedicated, out).await
        }
    }
}

async fn import_external(
    state: &Arc<LauncherState>,
    instances: Vec<ExternalInstance>,
    move_files: bool,
    improve: bool,
    scan: Vec<PathBuf>,
    out: &Output,
) -> anyhow::Result<()> {
    let _slot = oneclient_core::busy::ImportSlot::claim()?;
    let screened = if improve {
        out.line("Checking the mods for known problems");
        oneclient_core::screen_external_instances(state, &instances).await?
    } else {
        Vec::new()
    };

    for instance in instances {
        if let Some(loader) = &instance.unsupported_loader {
            out.error(warn(format!(
                "Skipping {}, it uses {loader} which OneClient can't run",
                instance.name
            )));
            continue;
        }

        let check = oneclient_core::find_unpublished_mods(state, &instance).await;
        if !check.mods.is_empty() {
            out.error(warn(format!(
                "{}: {} mod{} not on Modrinth or CurseForge, nobody has checked them and they could contain malware. Only import instances from people you trust: {}",
                instance.name,
                check.mods.len(),
                if check.mods.len() == 1 { "" } else { "s" },
                check
                    .mods
                    .iter()
                    .map(|file| file.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        } else if check.unconfirmed > 0 {
            out.error(warn(format!(
                "{}: could not reach Modrinth or CurseForge to confirm {} mod{} are published there",
                instance.name,
                check.unconfirmed,
                if check.unconfirmed == 1 { "" } else { "s" }
            )));
        }

        let mut choices = ImportChoices {
            move_files,
            ..ImportChoices::default()
        };
        let mut replacements: Vec<(ProviderId, String, String)> = Vec::new();
        let flagged = screened
            .iter()
            .find(|screened| screened.game_dir == instance.game_dir)
            .map(|screened| screened.flagged.as_slice())
            .unwrap_or_default();
        for item in flagged {
            let pick = item
                .alternatives
                .iter()
                .find(|alternative| alternative.already_installed)
                .or_else(|| {
                    item.alternatives
                        .iter()
                        .find(|alternative| alternative.resolved.version_number.is_some())
                });
            match pick {
                Some(alternative) => {
                    choices.skip.insert(item.hash.clone());
                    out.line(format!(
                        "  Swapping {} for {}",
                        item.name, alternative.resolved.name
                    ));
                    if !alternative.already_installed {
                        replacements.push((
                            alternative.resolved.provider,
                            alternative.resolved.project_id.clone(),
                            alternative.resolved.name.clone(),
                        ));
                    }
                }
                None => out.line(dim(format!(
                    "  Keeping {}, there is nothing better to swap it for",
                    item.name
                ))),
            }
        }

        let session = oneclient_events::GroupedProgressSession::start(
            &state.services.events,
            format!("Importing {}", instance.name),
        );
        let outcome =
            oneclient_core::import_external_instance(state, &instance, &choices, Some(&session))
                .await;
        if outcome.is_ok() {
            session.finish();
        } else {
            session.abort();
        }
        clusters_changed(state);
        let report = match outcome {
            Ok(report) => report,
            Err(err) => {
                out.error(format!("Could not import {}: {err:#}", instance.name));
                continue;
            }
        };

        out.line(format!(
            "{} Imported {} from {} as {}",
            good("✓"),
            bold(&instance.name),
            instance.launcher.display_name(),
            report.cluster_name
        ));
        if !report.content_failed.is_empty() {
            out.error(warn(format!(
                "  Could not bring over: {}",
                report.content_failed.join(", ")
            )));
        }
        if report.mods_skipped > 0 {
            out.line(dim(format!(
                "  Left out {} mods, the instance had no mod loader",
                report.mods_skipped
            )));
        }
        if !report.files_skipped.is_empty() {
            out.line(dim(format!(
                "  Could not copy: {}",
                report.files_skipped.join(", ")
            )));
        }
        if !report.parts_skipped.is_empty() {
            out.line(dim(format!(
                "  Not carried over: {}",
                report.parts_skipped.join(", ")
            )));
        }
        if let Some(major) = report.java_pinned {
            out.line(dim(format!(
                "  Pinned to Java {major}, which it ran on before"
            )));
        }
        if let Some(major) = report.jvm_args_dropped_for {
            out.line(dim(format!(
                "  Java {major} could not be set up, so its JVM arguments were left out"
            )));
        }

        for (provider, project_id, name) in replacements {
            let version = oneclient_core::alternative_version_id(
                state,
                report.cluster_id,
                provider,
                &project_id,
            )
            .await
            .ok()
            .flatten();
            let Some(version_id) = version else {
                out.error(format!("  {name} has no version for this cluster"));
                continue;
            };
            match oneclient_core::install::install_package(
                state,
                provider,
                &project_id,
                &version_id,
                report.cluster_id,
                None,
                true,
            )
            .await
            {
                Ok(install) => {
                    if let Some(session_id) = install.session_id {
                        super::end_progress(state, session_id);
                    }
                    if let Err(err) = install.result {
                        out.error(format!("  Could not add {name}: {err:#}"));
                    }
                }
                Err(_) => out.error(format!("  Could not add {name}")),
            }
        }

        let retry = "the same import again";
        super::modpack::blocked(
            state,
            report.cluster_id,
            oneclient_content::modpacks::MODPACK_BUNDLE_NAME,
            retry,
            &report.blocked,
            scan.clone(),
            out,
        )
        .await?;
    }
    clusters_changed(state);
    Ok(())
}
