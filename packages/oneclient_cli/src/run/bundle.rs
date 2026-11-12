use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::Subcommand;
use oneclient_common::domain::{GameLoader, ProviderId};
use oneclient_content::bundles::{BundleArchive, BundleFile, BundleFileKind, FileUpdateStatus};
use oneclient_core::{Cluster, LauncherState};
use serde::{Deserialize, Serialize};

use super::settings::parse_bool;
use super::{clusters_changed, find_cluster};
use crate::output::{bold, dim, good, heading, table, warn};
use crate::{Loader, Output};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum BundleCommand {
    /// Fetch the newest bundle catalog and set up the clusters it adds
    Sync,
    /// List the bundles offered for a Minecraft version and loader
    Available {
        #[arg(short = 'V', long = "version")]
        mc_version: String,
        #[arg(short, long, value_enum, default_value_t = Loader::Fabric)]
        loader: Loader,
    },
    /// List a cluster's bundles and what each one installs
    #[command(visible_alias = "ls")]
    List { cluster: String },
    /// Take or drop an optional bundle
    Choose {
        cluster: String,
        bundle: String,
        #[arg(value_parser = parse_bool, action = clap::ArgAction::Set)]
        take: bool,
    },
    /// Turn one of a bundle's packages on or off
    Toggle {
        cluster: String,
        bundle: String,
        /// Package file name or project id
        package: String,
        #[arg(value_parser = parse_bool, action = clap::ArgAction::Set)]
        enabled: bool,
    },
    /// Install every package of a bundle
    Install { cluster: String, bundle: String },
    /// List the bundle changes waiting for a cluster
    Updates { cluster: String },
    /// Apply the waiting bundle changes now instead of at the next launch
    Update { cluster: String },
    /// List, take or turn down the optional mods bundles offer
    Optional {
        cluster: String,
        /// Packages to turn on
        #[arg(long)]
        enable: Vec<String>,
        /// Packages to stop offering
        #[arg(long)]
        skip: Vec<String>,
        /// Turn on every offered mod
        #[arg(long, conflicts_with_all = ["enable", "skip", "skip_all"])]
        enable_all: bool,
        /// Stop offering every mod
        #[arg(long, conflicts_with_all = ["enable", "skip", "enable_all"])]
        skip_all: bool,
    },
}

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: BundleCommand,
    out: &Output,
) -> anyhow::Result<()> {
    let ctx = state.services.content();
    match command {
        BundleCommand::Sync => {
            out.line("Syncing the bundle catalog");
            state.bundles.sync(&ctx).await?;
            let migrated = oneclient_core::clusters::apply_remote_migrations(state).await?;
            let added = oneclient_core::clusters::ensure_from_bundles(state).await?;
            clusters_changed(state);
            out.line(format!(
                "{} Synced, {} new cluster{}, {migrated} migrated",
                good("✓"),
                added.len(),
                if added.len() == 1 { "" } else { "s" }
            ));
            Ok(())
        }
        BundleCommand::Available { mc_version, loader } => {
            let loader = GameLoader::from(loader);
            let archives = state
                .bundles
                .archives_for(&ctx, &mc_version, loader)
                .await?;
            if archives.is_empty() {
                out.line(format!("No bundles for {loader} {mc_version}"));
                return Ok(());
            }
            let rows: Vec<Vec<String>> = archives
                .iter()
                .map(|archive| {
                    vec![
                        bold(&archive.manifest.name),
                        display_name(archive),
                        if archive.manifest.enabled {
                            dim("always")
                        } else {
                            "optional".to_string()
                        },
                        archive.manifest.files.len().to_string(),
                    ]
                })
                .collect();
            out.line(table(&["NAME", "CATEGORY", "TAKEN", "FILES"], &rows));
            Ok(())
        }
        BundleCommand::List { cluster } => {
            let cluster = bundle_cluster(state, &cluster).await?;
            let bundles = oneclient_core::get_bundles_with_update_status(
                cluster.id,
                state.bundles.as_ref(),
                &ctx,
            )
            .await?;
            if bundles.is_empty() {
                out.line(format!("{} has no bundles", cluster.name));
                return Ok(());
            }
            let overrides = oneclient_core::list_cluster_bundle_overrides(cluster.id, &ctx).await?;
            let ids = managed_ids(
                bundles
                    .iter()
                    .flat_map(|bundle| bundle.files.iter().map(|(file, _)| file)),
            );
            let names = package_names(state, ids).await;

            let mut lines = Vec::new();
            for bundle in &bundles {
                lines.push(format!(
                    "{} {}{}",
                    heading(display_name(&bundle.archive)),
                    dim(&bundle.archive.manifest.name),
                    if bundle.has_updates {
                        format!(" {}", warn("updates waiting"))
                    } else {
                        String::new()
                    }
                ));
                let rows: Vec<Vec<String>> = bundle
                    .files
                    .iter()
                    .filter(|(file, _)| !file.hidden)
                    .map(|(file, status)| {
                        let package_id = file.kind.package_id();
                        let user = overrides
                            .iter()
                            .find(|(name, id, _)| {
                                name == &bundle.archive.manifest.name && id == &package_id
                            })
                            .map(|(_, _, kind)| kind.as_str());
                        vec![
                            names
                                .get(&package_id)
                                .cloned()
                                .unwrap_or_else(|| file.display_name()),
                            match status {
                                FileUpdateStatus::NotInstalled => dim("not installed"),
                                FileUpdateStatus::RemovedByUser => dim("removed"),
                                FileUpdateStatus::UpToDate => good("installed"),
                                FileUpdateStatus::UpdateAvailable { .. } => warn("update waiting"),
                            },
                            match (user, file.enabled) {
                                (Some(kind), _) => kind.to_string(),
                                (None, true) => String::new(),
                                (None, false) => dim("optional"),
                            },
                            dim(package_id),
                        ]
                    })
                    .collect();
                lines.push(table(&["PACKAGE", "STATE", "CHOICE", "ID"], &rows));
                lines.push(String::new());
            }
            out.line(lines.join("\n").trim_end().to_string());
            Ok(())
        }
        BundleCommand::Choose {
            cluster,
            bundle,
            take,
        } => {
            let cluster = bundle_cluster(state, &cluster).await?;
            let archive = find_bundle(state, &cluster, &bundle).await?;
            oneclient_content::bundles::set_bundle_choices(
                cluster.id,
                &[(archive.manifest.name.clone(), take)],
                &ctx,
            )
            .await?;
            for file in archive.manifest.files.iter().filter(|file| file.enabled) {
                oneclient_core::set_bundle_package_enabled(
                    cluster.id,
                    &archive.manifest.name,
                    &file.kind.package_id(),
                    take,
                    true,
                    &ctx,
                )
                .await?;
            }
            apply(state, &cluster, out).await?;
            out.line(format!(
                "{} {} for {}",
                if take { "Took" } else { "Dropped" },
                bold(display_name(&archive)),
                cluster.name
            ));
            Ok(())
        }
        BundleCommand::Toggle {
            cluster,
            bundle,
            package,
            enabled,
        } => {
            let cluster = bundle_cluster(state, &cluster).await?;
            let archive = find_bundle(state, &cluster, &bundle).await?;
            let names = package_names(state, managed_ids(archive.manifest.files.iter())).await;
            let file = archive
                .manifest
                .files
                .iter()
                .find(|file| {
                    let id = file.kind.package_id();
                    id == package
                        || file.display_name().eq_ignore_ascii_case(&package)
                        || names
                            .get(&id)
                            .is_some_and(|name| name.eq_ignore_ascii_case(&package))
                })
                .with_context(|| {
                    format!(
                        "{} has no package \"{package}\", see `bundle list`",
                        display_name(&archive)
                    )
                })?;
            oneclient_core::set_bundle_package_enabled(
                cluster.id,
                &archive.manifest.name,
                &file.kind.package_id(),
                enabled,
                file.enabled,
                &ctx,
            )
            .await?;
            apply(state, &cluster, out).await?;
            let id = file.kind.package_id();
            out.line(format!(
                "{} {}",
                if enabled { "Enabled" } else { "Disabled" },
                bold(
                    names
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| file.display_name())
                )
            ));
            Ok(())
        }
        BundleCommand::Install { cluster, bundle } => {
            let cluster = bundle_cluster(state, &cluster).await?;
            let archive = find_bundle(state, &cluster, &bundle).await?;
            let installed = oneclient_core::install_bundle(
                cluster.id,
                &archive.manifest.name,
                false,
                state.bundles.as_ref(),
                &ctx,
            )
            .await?;
            clusters_changed(state);
            out.line(format!(
                "{} Installed {} files from {}",
                good("✓"),
                installed.len(),
                display_name(&archive)
            ));
            Ok(())
        }
        BundleCommand::Updates { cluster } => {
            let cluster = bundle_cluster(state, &cluster).await?;
            let check =
                oneclient_core::check_bundle_updates(cluster.id, state.bundles.as_ref(), &ctx)
                    .await?;
            let mut rows: Vec<Vec<String>> = Vec::new();
            for update in &check.updates_available {
                rows.push(vec![
                    update.new_file.display_name(),
                    warn("update"),
                    dim(&update.bundle_name),
                ]);
            }
            for addition in &check.additions_available {
                rows.push(vec![
                    addition.new_file.display_name(),
                    good("new"),
                    dim(&addition.bundle_name),
                ]);
            }
            for removal in &check.removals_available {
                rows.push(vec![
                    removal.package_id.clone(),
                    dim("removed"),
                    dim(&removal.bundle_name),
                ]);
            }
            if rows.is_empty() {
                out.line(format!("{}'s bundles are up to date", cluster.name));
            } else {
                out.line(table(&["PACKAGE", "CHANGE", "BUNDLE"], &rows));
            }
            let offered =
                oneclient_core::pending_optional_mods(cluster.id, state.bundles.as_ref(), &ctx)
                    .await
                    .map(|pending| pending.len())
                    .unwrap_or(0);
            if offered > 0 {
                out.line(dim(format!(
                    "{offered} optional mods on offer, see `bundle optional {}`",
                    cluster.id
                )));
            }
            Ok(())
        }
        BundleCommand::Update { cluster } => {
            let cluster = bundle_cluster(state, &cluster).await?;
            apply(state, &cluster, out).await
        }
        BundleCommand::Optional {
            cluster,
            enable,
            skip,
            enable_all,
            skip_all,
        } => optional(state, &cluster, enable, skip, enable_all, skip_all, out).await,
    }
}

fn display_name(archive: &BundleArchive) -> String {
    let category = archive.manifest.category.trim();
    if category.is_empty() {
        archive.manifest.name.clone()
    } else {
        category.to_string()
    }
}

async fn bundle_cluster(state: &LauncherState, query: &str) -> anyhow::Result<Cluster> {
    let cluster = find_cluster(state, query).await?;
    if !cluster.uses_bundles() {
        bail!("{} does not use bundles", cluster.name);
    }
    Ok(cluster)
}

async fn find_bundle(
    state: &LauncherState,
    cluster: &Cluster,
    query: &str,
) -> anyhow::Result<BundleArchive> {
    state
        .bundles
        .archives_for(
            &state.services.content(),
            &cluster.mc_version,
            cluster.mc_loader,
        )
        .await?
        .into_iter()
        .find(|archive| {
            archive.manifest.name.eq_ignore_ascii_case(query)
                || display_name(archive).eq_ignore_ascii_case(query)
        })
        .with_context(|| {
            format!(
                "no bundle \"{query}\" for {}, see `bundle list`",
                cluster.name
            )
        })
}

fn managed_ids<'a>(
    files: impl Iterator<Item = &'a BundleFile>,
) -> HashMap<ProviderId, Vec<String>> {
    let mut by_provider: HashMap<ProviderId, Vec<String>> = HashMap::new();
    for file in files {
        if let BundleFileKind::Managed {
            provider,
            project_id,
            ..
        } = &file.kind
        {
            by_provider
                .entry(*provider)
                .or_default()
                .push(project_id.clone());
        }
    }
    by_provider
}

async fn package_names(
    state: &LauncherState,
    by_provider: HashMap<ProviderId, Vec<String>>,
) -> HashMap<String, String> {
    let ctx = state.services.content();
    let mut names = HashMap::new();
    for (provider, ids) in by_provider {
        if let Ok(metas) =
            oneclient_content::packages::fetch_package_meta(&ctx, provider, &ids).await
        {
            names.extend(metas.into_iter().map(|(id, meta)| (id, meta.name)));
        }
    }
    names
}

async fn apply(state: &Arc<LauncherState>, cluster: &Cluster, out: &Output) -> anyhow::Result<()> {
    let result = oneclient_core::apply_bundle_updates(
        cluster.id,
        state.bundles.as_ref(),
        &state.services.content(),
        None,
    )
    .await?;
    clusters_changed(state);

    let changed = result.updates_applied.len()
        + result.additions_applied.len()
        + result.removals_applied.len();
    let failed =
        result.updates_failed.len() + result.additions_failed.len() + result.removals_failed.len();
    if changed > 0 {
        out.line(format!(
            "{} Applied {changed} bundle change{} to {}",
            good("✓"),
            if changed == 1 { "" } else { "s" },
            cluster.name
        ));
    }
    if failed > 0 {
        out.error(warn(format!(
            "{failed} bundle changes could not be applied"
        )));
    }
    if changed == 0 && failed == 0 {
        out.line(format!("{}'s bundles are up to date", cluster.name));
    }
    Ok(())
}

async fn optional(
    state: &Arc<LauncherState>,
    cluster: &str,
    enable: Vec<String>,
    skip: Vec<String>,
    enable_all: bool,
    skip_all: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let cluster = bundle_cluster(state, cluster).await?;
    let pending = oneclient_core::pending_optional_mods(
        cluster.id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await?;
    if pending.is_empty() {
        out.line(format!("{} has no optional mods on offer", cluster.name));
        return Ok(());
    }

    let ids = managed_ids(pending.iter().map(|p| &p.file));
    let names = package_names(state, ids).await;
    let label = |package_id: &str, file: &BundleFile| {
        names
            .get(package_id)
            .cloned()
            .unwrap_or_else(|| file.display_name())
    };
    let pick = |queries: &[String]| -> anyhow::Result<Vec<oneclient_core::install::OptionalMod>> {
        queries
            .iter()
            .map(|query| {
                pending
                    .iter()
                    .find(|p| {
                        &p.package_id == query
                            || label(&p.package_id, &p.file).eq_ignore_ascii_case(query)
                    })
                    .map(|p| (cluster.id, (p.bundle_name.clone(), p.package_id.clone())))
                    .with_context(|| format!("\"{query}\" is not on offer for {}", cluster.name))
            })
            .collect()
    };
    let every: Vec<oneclient_core::install::OptionalMod> = pending
        .iter()
        .map(|p| (cluster.id, (p.bundle_name.clone(), p.package_id.clone())))
        .collect();

    let to_enable = if enable_all {
        every.clone()
    } else {
        pick(&enable)?
    };
    let to_skip = if skip_all { every } else { pick(&skip)? };

    if to_enable.is_empty() && to_skip.is_empty() {
        let rows: Vec<Vec<String>> = pending
            .iter()
            .map(|p| {
                vec![
                    bold(label(&p.package_id, &p.file)),
                    dim(&p.bundle_name),
                    dim(&p.package_id),
                ]
            })
            .collect();
        out.line(table(&["MOD", "BUNDLE", "ID"], &rows));
        out.line(dim(
            "Turn them on with --enable <mod> or --enable-all, or stop offering them with --skip",
        ));
        return Ok(());
    }

    if !to_skip.is_empty() {
        oneclient_core::install::skip_optional_mods(state, &to_skip).await;
        out.line(format!("Stopped offering {} mods", to_skip.len()));
    }
    if !to_enable.is_empty() {
        let outcome = oneclient_core::install::enable_optional_mods(state, &to_enable).await;
        out.line(format!("{} Added {} mods", good("✓"), outcome.installed));
        if outcome.failed > 0 {
            out.error(warn(format!("{} could not be installed", outcome.failed)));
        }
    }
    clusters_changed(state);
    Ok(())
}
