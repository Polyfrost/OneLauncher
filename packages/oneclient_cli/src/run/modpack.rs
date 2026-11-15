use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::Subcommand;
use oneclient_common::domain::{ContentType, ProviderId};
use oneclient_content::modpacks::{BlockedFile, MODPACK_BUNDLE_NAME, ModpackUpdateStatus};
use oneclient_core::LauncherState;
use oneclient_core::busy;
use oneclient_core::clusters::{
    ExistingPack, LoaderPlan, ModpackCluster, ModpackSource, PreparedModpack,
};
use oneclient_events::GroupedProgressSession;
use serde::{Deserialize, Serialize};

use super::{clusters_changed, find_cluster};
use crate::output::{accent, bold, bytes, dim, good, heading, table, warn};
use crate::{Output, Provider};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum ModpackCommand {
    /// Install a modpack as a new cluster, from a .mrpack or CurseForge .zip, or a project id
    Install {
        /// File path, or a project id or slug
        source: String,
        #[arg(short, long, value_enum, default_value_t = Provider::Modrinth)]
        provider: Provider,
        /// Version id, the newest when left out
        #[arg(long = "version")]
        version_id: Option<String>,
        /// Name for the new cluster
        #[arg(short, long)]
        name: Option<String>,
        /// Folders to look in for files CurseForge makes you download by hand
        #[arg(long)]
        scan: Vec<PathBuf>,
        /// Install even when the modpack is known to cause problems
        #[arg(long)]
        force: bool,
    },
    /// Add a modpack on top of an existing cluster with the same version and loader
    Add {
        cluster: String,
        /// File path, or a project id or slug
        source: String,
        #[arg(short, long, value_enum, default_value_t = Provider::Modrinth)]
        provider: Provider,
        /// Version id, the newest that fits the cluster when left out
        #[arg(long = "version")]
        version_id: Option<String>,
        /// Keep a pack with the same name next to this one instead of replacing it
        #[arg(long)]
        keep_both: bool,
        /// Keep mods known to cause problems instead of leaving them out
        #[arg(long, conflicts_with = "replace_flagged")]
        keep_flagged: bool,
        /// Swap mods known to cause problems for their suggested alternatives
        #[arg(long)]
        replace_flagged: bool,
        #[arg(long)]
        scan: Vec<PathBuf>,
        /// Add even when the modpack is known to cause problems
        #[arg(long)]
        force: bool,
    },
    /// List the modpacks added on top of a cluster
    Added { cluster: String },
    /// Take an added modpack and its files back out of a cluster
    #[command(visible_alias = "rm")]
    Remove { cluster: String, pack: String },
    /// Show which modpack a cluster comes from and whether an update is out
    Check { cluster: String },
    /// Update a modpack cluster, to the newest version when none is given
    Update {
        cluster: String,
        #[arg(long = "version")]
        version_id: Option<String>,
        #[arg(long)]
        scan: Vec<PathBuf>,
    },
    /// Download the modpack's missing files again and pick up manual downloads
    Repair {
        cluster: String,
        #[arg(long)]
        scan: Vec<PathBuf>,
    },
}

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: ModpackCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        ModpackCommand::Install {
            source,
            provider,
            version_id,
            name,
            scan,
            force,
        } => {
            let source = resolve_source(state, &source, provider.into(), version_id, None).await?;
            install(state, source, name, scan, force, out)
                .await
                .map(|_| ())
        }
        ModpackCommand::Add {
            cluster,
            source,
            provider,
            version_id,
            keep_both,
            keep_flagged,
            replace_flagged,
            scan,
            force,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let source =
                resolve_source(state, &source, provider.into(), version_id, Some(&cluster)).await?;
            let choices = AddChoices {
                existing: if keep_both {
                    ExistingPack::KeepBoth
                } else {
                    ExistingPack::Replace
                },
                keep_flagged,
                replace_flagged,
                force,
            };
            add(state, &cluster, source, choices, scan, out).await
        }
        ModpackCommand::Added { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let packs = oneclient_core::clusters::list_imported_modpacks(state, cluster.id).await?;
            if packs.is_empty() {
                out.line(format!("No modpacks were added to {}", cluster.name));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = packs
                .iter()
                .map(|pack| {
                    vec![
                        bold(&pack.name),
                        pack.version.clone(),
                        pack.provider
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| dim("file")),
                        dim(&pack.bundle_name),
                    ]
                })
                .collect();
            out.line(table(&["MODPACK", "VERSION", "SOURCE", "ID"], &rows));
            Ok(())
        }
        ModpackCommand::Remove { cluster, pack } => {
            let cluster = find_cluster(state, &cluster).await?;
            let _job = Job::claim(state, Some(&cluster))?;
            let packs = oneclient_core::clusters::list_imported_modpacks(state, cluster.id).await?;
            let found = packs
                .iter()
                .find(|p| p.bundle_name == pack || p.name.eq_ignore_ascii_case(&pack))
                .with_context(|| {
                    format!(
                        "{} has no added modpack \"{pack}\", see `modpack added`",
                        cluster.name
                    )
                })?;
            let left = oneclient_core::clusters::remove_imported_modpack(
                state,
                cluster.id,
                &found.bundle_name,
            )
            .await?;
            clusters_changed(state);
            out.line(format!(
                "Removed {} from {}",
                bold(&found.name),
                cluster.name
            ));
            if !left.is_empty() {
                out.error(warn(format!("Could not remove: {}", left.join(", "))));
            }
            Ok(())
        }
        ModpackCommand::Check { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let ctx = state.services.content();
            let Some(release) =
                oneclient_content::modpacks::cluster_modpack(cluster.id, &ctx).await?
            else {
                if cluster.linked_modpack_hash.is_some() {
                    out.line(format!(
                        "{} comes from a modpack that is not on Modrinth or CurseForge",
                        cluster.name
                    ));
                } else {
                    out.line(format!("{} is not a modpack", cluster.name));
                }
                return Ok(());
            };
            let project = match state.services.packages.get(release.provider) {
                Ok(provider) => provider
                    .get_project(&release.project_id, &ctx)
                    .await
                    .map(|project| project.name)
                    .unwrap_or_else(|_| release.name.clone()),
                Err(_) => release.name.clone(),
            };
            out.line(format!(
                "{} is {} {} from {}",
                bold(&cluster.name),
                accent(project),
                release.version,
                release.provider
            ));
            match oneclient_content::modpacks::check_modpack_update(cluster.id, &ctx).await? {
                ModpackUpdateStatus::Available(version) => out.line(format!(
                    "{} {} is out, update with `oneclient_cli modpack update {}`",
                    good("Update:"),
                    bold(&version.version_number),
                    cluster.id
                )),
                ModpackUpdateStatus::UpToDate => out.line("It is up to date"),
                ModpackUpdateStatus::Unpublished => out.line(dim("No published versions")),
            }
            Ok(())
        }
        ModpackCommand::Update {
            cluster,
            version_id,
            scan,
        } => {
            let (cluster, _job) = idle(state, &cluster).await?;
            let version_id = match version_id {
                Some(version_id) => version_id,
                None => match oneclient_content::modpacks::check_modpack_update(
                    cluster.id,
                    &state.services.content(),
                )
                .await?
                {
                    ModpackUpdateStatus::Available(version) => version.version_id,
                    ModpackUpdateStatus::UpToDate => {
                        out.line(format!("{} is up to date", cluster.name));
                        return Ok(());
                    }
                    ModpackUpdateStatus::Unpublished => {
                        bail!(
                            "{}'s modpack is not on Modrinth or CurseForge",
                            cluster.name
                        )
                    }
                },
            };

            let session = GroupedProgressSession::start(
                &state.services.events,
                format!("Updating {}", cluster.name),
            );
            let outcome = oneclient_core::clusters::update_modpack_cluster(
                state,
                cluster.id,
                &version_id,
                Some(&session),
            )
            .await;
            session.finish();
            clusters_changed(state);
            let retry = format!("`oneclient_cli modpack repair {}`", cluster.id);
            report(
                state,
                outcome?,
                MODPACK_BUNDLE_NAME,
                &retry,
                scan,
                "Updated",
                out,
            )
            .await
        }
        ModpackCommand::Repair { cluster, scan } => {
            let (cluster, _job) = idle(state, &cluster).await?;
            let session = GroupedProgressSession::start(
                &state.services.events,
                format!("Repairing {}", cluster.name),
            );
            let outcome =
                oneclient_core::clusters::repair_modpack_cluster(state, cluster.id, Some(&session))
                    .await;
            session.finish();
            clusters_changed(state);
            let retry = format!("`oneclient_cli modpack repair {}`", cluster.id);
            report(
                state,
                outcome?,
                MODPACK_BUNDLE_NAME,
                &retry,
                scan,
                "Repaired",
                out,
            )
            .await
        }
    }
}

async fn idle(
    state: &LauncherState,
    query: &str,
) -> anyhow::Result<(oneclient_core::Cluster, Job)> {
    let cluster = find_cluster(state, query).await?;
    if cluster.linked_modpack_hash.is_none() {
        bail!("{} is not a modpack", cluster.name);
    }
    let job = Job::claim(state, Some(&cluster))?;
    Ok((cluster, job))
}

pub(super) struct Job;

impl Job {
    pub(super) fn claim(
        state: &LauncherState,
        cluster: Option<&oneclient_core::Cluster>,
    ) -> anyhow::Result<Self> {
        if let Some(cluster) = cluster
            && let Some(busy) = busy::modpack_change_block(state, cluster.id)
        {
            bail!("{}: {busy}", cluster.name);
        }
        busy::claim_modpack_job(cluster.map(|cluster| cluster.id))?;
        Ok(Self)
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        busy::release_modpack_job();
    }
}

async fn resolve_source(
    state: &Arc<LauncherState>,
    source: &str,
    provider_id: ProviderId,
    version_id: Option<String>,
    cluster: Option<&oneclient_core::Cluster>,
) -> anyhow::Result<ModpackSource> {
    let path = PathBuf::from(source);
    if path.is_file() {
        return Ok(ModpackSource::File(
            dunce::canonicalize(&path).unwrap_or(path),
        ));
    }

    let ctx = state.services.content();
    let provider = state.services.packages.get(provider_id)?;
    let project = provider.get_project(source, &ctx).await?;
    if project.content_type != ContentType::Modpack {
        bail!(
            "{} is not a modpack, add it to a cluster with `oneclient_cli package install`",
            project.name
        );
    }
    let version_id = match (version_id, cluster) {
        (Some(version_id), _) => version_id,
        (None, Some(cluster)) => {
            oneclient_content::modpacks::latest_modpack_version(
                provider_id,
                &project.id,
                &cluster.mc_version,
                cluster.mc_loader,
                &ctx,
            )
            .await?
            .with_context(|| {
                format!(
                    "{} has no version for {} {}",
                    project.name, cluster.mc_loader, cluster.mc_version
                )
            })?
            .version_id
        }
        (None, None) => {
            provider
                .list_versions(&project.id, None, None, 0, 1, &ctx)
                .await?
                .items
                .into_iter()
                .next()
                .with_context(|| format!("{} has no versions", project.name))?
                .version_id
        }
    };
    Ok(ModpackSource::Provider {
        provider: provider_id,
        project_id: project.id,
        version_id,
    })
}

fn describe(prepared: &PreparedModpack, out: &Output) {
    let manifest = &prepared.manifest;
    let (name, version) = match &prepared.share {
        Some(shared) if shared.share.modpack.is_some() => (
            shared.share.name.clone(),
            format!("{} {}", manifest.name, manifest.version),
        ),
        Some(shared) => (shared.share.name.clone(), "shared cluster".to_string()),
        None => (manifest.name.clone(), manifest.version.clone()),
    };
    let summary = prepared.summary();
    out.line(format!(
        "{} {} {}",
        heading(name),
        version,
        dim(format!(
            "· Minecraft {} {}",
            manifest.mc_version, manifest.loader
        ))
    ));
    out.line(format!(
        "  {} mods, {} resource packs, {} shaders, {} other files, {} to download",
        summary.mods,
        summary.resource_packs,
        summary.shaders,
        summary.other_files,
        bytes(summary.download_bytes)
    ));
    if summary.manual > 0 {
        out.line(warn(format!(
            "  {} files have to be downloaded by hand",
            summary.manual
        )));
    }
    for notice in prepared.notices() {
        out.line(dim(format!("  {notice}")));
    }
    if !prepared.unlisted.is_empty() {
        out.error(warn(format!(
            "  {} mod{} not on Modrinth or CurseForge, nobody has checked them and they could contain malware. Only install this if you trust who gave it to you:",
            prepared.unlisted.len(),
            if prepared.unlisted.len() == 1 { "" } else { "s" }
        )));
        for unlisted in &prepared.unlisted {
            out.error(format!(
                "    {} {}",
                unlisted.name,
                dim(if unlisted.bundled {
                    "packed in the file"
                } else {
                    "downloaded from another site"
                })
            ));
        }
    }
    if prepared.mods_unchecked {
        out.error(warn(
            "  Could not check whether its mods are on Modrinth or CurseForge",
        ));
    } else if prepared.mods_unconfirmed > 0 {
        out.error(warn(format!(
            "  Could not reach Modrinth or CurseForge to confirm {} mod{} are published there",
            prepared.mods_unconfirmed,
            if prepared.mods_unconfirmed == 1 {
                ""
            } else {
                "s"
            }
        )));
    }
}

fn refuse_flagged(prepared: &PreparedModpack, force: bool, out: &Output) -> anyhow::Result<()> {
    let Some(flagged) = &prepared.flagged else {
        return Ok(());
    };
    let why = flagged
        .explanation
        .as_deref()
        .map(|why| format!(": {why}"))
        .unwrap_or_default();
    if !force {
        bail!(
            "{} is flagged as a problematic modpack{why}, add --force to install it anyway",
            prepared.manifest.name
        );
    }
    out.error(warn(format!(
        "{} is flagged as a problematic modpack{why}",
        prepared.manifest.name
    )));
    Ok(())
}

pub(super) async fn install(
    state: &Arc<LauncherState>,
    source: ModpackSource,
    name: Option<String>,
    scan: Vec<PathBuf>,
    force: bool,
    out: &Output,
) -> anyhow::Result<i64> {
    let _job = Job::claim(state, None)?;
    let session = GroupedProgressSession::start(&state.services.events, "Reading modpack");
    let prepared = oneclient_core::clusters::prepare_modpack(state, &source, Some(&session)).await;
    session.finish();
    let mut prepared = prepared?;
    if let Some(name) = name {
        prepared.instance_name = name;
    }

    describe(&prepared, out);
    refuse_flagged(&prepared, force, out)?;

    let cluster = oneclient_core::clusters::create_modpack_instance(state, &prepared).await?;
    busy::lock_modpack_job(cluster.id);
    clusters_changed(state);

    let session = GroupedProgressSession::start(
        &state.services.events,
        format!("Installing {}", cluster.name),
    );
    let outcome = oneclient_core::clusters::install_modpack_instance(
        state,
        cluster.id,
        &prepared,
        Some(&session),
    )
    .await;
    session.finish();

    match outcome {
        Ok(outcome) => {
            clusters_changed(state);
            let retry = format!("`oneclient_cli modpack repair {}`", outcome.cluster.id);
            let cluster_id = outcome.cluster.id;
            report(
                state,
                outcome,
                MODPACK_BUNDLE_NAME,
                &retry,
                scan,
                "Installed",
                out,
            )
            .await?;
            Ok(cluster_id)
        }
        Err(err) => {
            let removed = state.clusters.delete(cluster.id, true).await;
            clusters_changed(state);
            match removed {
                Ok(()) => bail!("{err:#}, the unfinished cluster was removed"),
                Err(_) => bail!(
                    "{err:#}, finish it with `oneclient_cli modpack repair {}`",
                    cluster.id
                ),
            }
        }
    }
}

struct AddChoices {
    existing: ExistingPack,
    keep_flagged: bool,
    replace_flagged: bool,
    force: bool,
}

async fn add(
    state: &Arc<LauncherState>,
    cluster: &oneclient_core::Cluster,
    source: ModpackSource,
    choices: AddChoices,
    scan: Vec<PathBuf>,
    out: &Output,
) -> anyhow::Result<()> {
    let _job = Job::claim(state, Some(cluster))?;

    let session = GroupedProgressSession::start(&state.services.events, "Reading modpack");
    let import = oneclient_core::clusters::prepare_modpack_import(
        state,
        cluster.id,
        &source,
        Some(&session),
    )
    .await;
    session.finish();
    let import = import?;

    describe(&import.prepared, out);
    refuse_flagged(&import.prepared, choices.force, out)?;
    match &import.loader {
        LoaderPlan::Keep => {}
        LoaderPlan::Raise { from, to } => out.line(format!(
            "  {} will be updated from {from} to {to}",
            cluster.mc_loader
        )),
        LoaderPlan::Behind { current, needed } => out.error(warn(format!(
            "  The pack was made for {} {needed}, {} has {current}",
            cluster.mc_loader, cluster.name
        ))),
    }
    if import.shared_game_dir {
        out.line(dim(
            "  This cluster shares its game folder, so the pack's configs apply to the others too",
        ));
    }
    if let Some(existing) = import.existing() {
        out.line(dim(format!(
            "  {} {} is already added, {}",
            existing.name,
            existing.version,
            if choices.existing == ExistingPack::KeepBoth {
                "keeping both"
            } else {
                "replacing it"
            }
        )));
    } else if let Some(previous) = &import.previous {
        out.line(dim(format!("  Updating from {}", previous.version)));
    }

    let mut skipped = HashSet::new();
    let mut replacements = Vec::new();
    for flagged in &import.flagged {
        let why = flagged
            .explanation
            .as_deref()
            .map(|why| {
                console::strip_ansi_codes(&crate::markdown::render(why))
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_else(|| "known to cause problems".to_string());
        if choices.keep_flagged {
            out.error(warn(format!("  Keeping {}, {why}", flagged.name)));
            continue;
        }
        skipped.insert(flagged.package_id.clone());
        let alternative = flagged
            .alternatives
            .iter()
            .find(|alternative| alternative.version_id.is_some())
            .filter(|_| choices.replace_flagged);
        match alternative {
            Some(alternative) => {
                out.line(format!(
                    "  Swapping {} for {}, {why}",
                    flagged.name, alternative.name
                ));
                replacements.push(alternative.clone());
            }
            None => out.line(format!("  Leaving out {}, {why}", flagged.name)),
        }
    }

    let bundle_name = import.target_bundle_name(choices.existing).to_string();
    let session = GroupedProgressSession::start(
        &state.services.events,
        format!("Adding {}", import.pack_name()),
    );
    let outcome = oneclient_core::clusters::import_modpack_into_cluster(
        state,
        &import,
        &skipped,
        choices.existing,
        Some(&session),
    )
    .await;
    session.finish();
    clusters_changed(state);
    let outcome = outcome?;

    let retry = "the same `oneclient_cli modpack add` command again";
    report(
        state,
        outcome,
        &bundle_name,
        retry,
        scan,
        "Added the pack to",
        out,
    )
    .await?;

    for alternative in replacements {
        let Some(version_id) = &alternative.version_id else {
            continue;
        };
        match oneclient_core::install::install_package(
            state,
            alternative.provider,
            &alternative.project_id,
            version_id,
            cluster.id,
            None,
            false,
        )
        .await
        {
            Ok(install) => {
                if let Some(session_id) = install.session_id {
                    super::end_progress(state, session_id);
                }
                match install.result {
                    Ok(name) => out.line(format!("Added {}", bold(name))),
                    Err(err) => out.error(format!("Could not add {}: {err:#}", alternative.name)),
                }
            }
            Err(flagged) => out.error(format!("{} is flagged too, left out", flagged.name)),
        }
    }
    clusters_changed(state);
    Ok(())
}

async fn report(
    state: &Arc<LauncherState>,
    outcome: ModpackCluster,
    bundle_name: &str,
    retry: &str,
    scan: Vec<PathBuf>,
    verb: &str,
    out: &Output,
) -> anyhow::Result<()> {
    let ModpackCluster { cluster, report } = outcome;
    out.line(format!(
        "{} {verb} {} with {} files",
        good("✓"),
        bold(&cluster.name),
        report.installed
    ));
    if !report.failed.is_empty() {
        out.error(warn(format!("Could not add: {}", report.failed.join(", "))));
    }
    if !report.kept.is_empty() {
        out.line(dim(format!(
            "Kept your versions of {}",
            report.kept.join(", ")
        )));
    }
    if !report.disabled_duplicates.is_empty() {
        out.line(dim(format!(
            "Turned off duplicates: {}",
            report.disabled_duplicates.join(", ")
        )));
    }
    for notice in &report.notices {
        out.line(dim(notice));
    }
    blocked(
        state,
        cluster.id,
        bundle_name,
        retry,
        &report.blocked,
        scan,
        out,
    )
    .await
}

pub(super) async fn blocked(
    state: &LauncherState,
    cluster_id: i64,
    bundle_name: &str,
    retry: &str,
    files: &[BlockedFile],
    scan: Vec<PathBuf>,
    out: &Output,
) -> anyhow::Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    let locations = if scan.is_empty() {
        directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(PathBuf::from))
            .into_iter()
            .collect()
    } else {
        scan
    };
    let found = oneclient_content::modpacks::find_blocked_downloads(&locations, files).await;
    if !found.is_empty() {
        oneclient_content::modpacks::import_blocked_files(
            cluster_id,
            bundle_name,
            &found,
            &state.services.content(),
        )
        .await?;
        clusters_changed(state);
        out.line(format!(
            "Added {} downloaded file{}",
            found.len(),
            if found.len() == 1 { "" } else { "s" }
        ));
    }

    let added: HashSet<&str> = found.iter().map(|(_, file)| file.sha1.as_str()).collect();
    let missing: Vec<&BlockedFile> = files
        .iter()
        .filter(|file| !added.contains(file.sha1.as_str()))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }

    out.line(warn(format!(
        "\nCurseForge does not allow {} file{} to be downloaded for you. Download {}, then run {retry}:",
        missing.len(),
        if missing.len() == 1 { "" } else { "s" },
        if missing.len() == 1 { "it" } else { "them" },
    )));
    for file in missing {
        out.line(format!(
            "  {} {} {}",
            bold(&file.project_name),
            dim(&file.file_name),
            file.page_url.as_deref().map(accent).unwrap_or_default()
        ));
    }
    Ok(())
}
