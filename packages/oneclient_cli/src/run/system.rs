use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use oneclient_core::{GameVersionKind, LauncherState};
use oneclient_java::JavaVendor;
use serde::{Deserialize, Serialize};

use crate::Output;
use crate::output::{accent, bold, dim, good, table};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum JavaCommand {
    /// List installed Java runtimes
    #[command(visible_alias = "ls")]
    List,
    /// Download a Java runtime
    Install {
        major: u32,
        /// Vendor to download from, the first that has it when left out
        #[arg(long, value_enum)]
        vendor: Option<JavaVendorArg>,
    },
    /// List the Java versions a vendor offers
    Available {
        #[arg(value_enum)]
        vendor: JavaVendorArg,
    },
    /// Add a Java runtime from a folder on disk
    Add { folder: PathBuf },
    /// Remove a Java runtime, deleting it when OneClient downloaded it
    #[command(visible_alias = "rm")]
    Remove {
        /// Its path, or its major version when only one is installed
        runtime: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum JavaVendorArg {
    Zulu,
    Adoptium,
    Corretto,
    Liberica,
    Microsoft,
    Mojang,
}

impl From<JavaVendorArg> for JavaVendor {
    fn from(vendor: JavaVendorArg) -> Self {
        match vendor {
            JavaVendorArg::Zulu => Self::Zulu,
            JavaVendorArg::Adoptium => Self::Adoptium,
            JavaVendorArg::Corretto => Self::Corretto,
            JavaVendorArg::Liberica => Self::Liberica,
            JavaVendorArg::Microsoft => Self::Microsoft,
            JavaVendorArg::Mojang => Self::Mojang,
        }
    }
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct VersionsArgs {
    /// Show the loaders for this Minecraft version
    pub mc_version: Option<String>,
    /// Include snapshots and old versions
    #[arg(short, long)]
    pub all: bool,
}

const LISTED_LOADER_VERSIONS: usize = 5;

pub(super) async fn java(
    state: &Arc<LauncherState>,
    command: JavaCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        JavaCommand::List => {
            let mut runtimes = state.java.list_runtimes().await?;
            if runtimes.is_empty() {
                out.line("No Java runtimes yet, they download on first launch");
                return Ok(());
            }
            runtimes.sort_by_key(|r| std::cmp::Reverse(r.major));
            let rows: Vec<Vec<String>> = runtimes
                .iter()
                .map(|runtime| {
                    vec![
                        bold(runtime.major),
                        runtime.version.clone(),
                        runtime.vendor.to_string(),
                        if runtime.is_jdk { "jdk" } else { "jre" }.to_string(),
                        dim(&runtime.absolute_path),
                    ]
                })
                .collect();
            out.line(table(
                &["MAJOR", "VERSION", "VENDOR", "KIND", "PATH"],
                &rows,
            ));
            Ok(())
        }
        JavaCommand::Install { major, vendor } => {
            out.line(format!("Downloading Java {major}"));
            let progress = oneclient_events::GroupedProgressSession::start(
                &state.services.events,
                format!("Installing Java {major}"),
            );
            let installed = match vendor {
                Some(vendor) => state.java.install_runtime_from(&vendor.into(), major).await,
                None => state.java.install_runtime(major, Some(&progress)).await,
            };
            progress.finish();
            let runtime = installed?;
            java_changed(state);
            out.line(format!(
                "{} Installed {} {} at {}",
                good("✓"),
                runtime.vendor,
                runtime.version,
                accent(&runtime.absolute_path)
            ));
            Ok(())
        }
        JavaCommand::Available { vendor } => {
            let vendor = vendor.into();
            let mut available = state.java.available_versions(&vendor).await?;
            available.sort_by_key(|java| std::cmp::Reverse(java.major));
            let runtimes = state.java.list_runtimes().await?;
            let rows: Vec<Vec<String>> = available
                .iter()
                .map(|java| {
                    let installed = runtimes
                        .iter()
                        .any(|runtime| runtime.vendor == vendor && runtime.major == java.major);
                    let mut package = dim(&java.package.name);
                    if installed {
                        package.push_str(&format!(" {}", good("installed")));
                    }
                    vec![bold(java.major), package]
                })
                .collect();
            out.line(table(&["MAJOR", "PACKAGE"], &rows));
            Ok(())
        }
        JavaCommand::Add { folder } => {
            let runtime = state
                .java
                .add_custom_runtime(dunce::canonicalize(&folder).unwrap_or(folder))
                .await?;
            java_changed(state);
            out.line(format!(
                "Added {} {} {}",
                runtime.vendor,
                runtime.version,
                dim(&runtime.absolute_path)
            ));
            Ok(())
        }
        JavaCommand::Remove { runtime } => {
            let runtimes = state.java.list_runtimes().await?;
            let target = match runtime.parse::<u32>() {
                Ok(major) => {
                    let matching: Vec<_> = runtimes.iter().filter(|r| r.major == major).collect();
                    match matching.as_slice() {
                        [only] => (*only).clone(),
                        [] => bail!("no Java {major} is installed"),
                        _ => bail!(
                            "several Java {major} runtimes are installed, pass the path instead"
                        ),
                    }
                }
                Err(_) => runtimes
                    .iter()
                    .find(|r| r.absolute_path == runtime)
                    .cloned()
                    .with_context(|| format!("no Java runtime at {runtime}, see `java list`"))?,
            };

            let profiles =
                oneclient_cluster::profiles::list_named_profiles(&state.services.db).await?;
            let clusters = state.clusters.list().await?;
            let pinned = oneclient_cluster::profiles::clusters_pinned_to_java(
                &profiles,
                &clusters,
                &target.absolute_path,
            );
            if !pinned.is_empty() {
                bail!(
                    "{} uses this runtime, change its java setting first",
                    pinned.join(", ")
                );
            }

            state.java.remove_runtime(&target.absolute_path).await?;
            java_changed(state);
            out.line(format!("Removed {} {}", target.vendor, target.version));
            Ok(())
        }
    }
}

fn java_changed(state: &LauncherState) {
    state
        .services
        .events
        .signal(oneclient_events::Signal::JavaChanged);
}

pub(super) async fn versions(
    state: &Arc<LauncherState>,
    args: VersionsArgs,
    out: &Output,
) -> anyhow::Result<()> {
    let mut metadata = state.metadata.lock().await;
    let ctx = state.services.mc();

    let Some(mc_version) = args.mc_version else {
        let versions = oneclient_core::get_version_ids(&mut metadata, &ctx).await?;
        let lines: Vec<String> = versions
            .iter()
            .filter(|v| args.all || matches!(v.kind, GameVersionKind::Release))
            .map(|v| match v.kind {
                GameVersionKind::Release => v.id.clone(),
                _ => format!("{} {}", v.id, dim(format!("{:?}", v.kind).to_lowercase())),
            })
            .collect();
        out.page(lines.join("\n"));
        return Ok(());
    };

    let loaders = oneclient_core::get_loaders_for_version(&mut metadata, &ctx, &mc_version).await?;
    let mut rows = Vec::new();
    for loader in loaders.into_iter().filter(|loader| loader.is_modded()) {
        let versions =
            oneclient_core::get_loader_versions(&mut metadata, &ctx, &mc_version, loader)
                .await
                .unwrap_or_default();
        let shown: Vec<&str> = versions
            .iter()
            .take(LISTED_LOADER_VERSIONS)
            .map(String::as_str)
            .collect();
        rows.push(vec![
            bold(loader),
            shown.join(", "),
            dim(format!("{} versions", versions.len())),
        ]);
    }

    if rows.is_empty() {
        out.line(format!("Minecraft {mc_version} has no mod loaders"));
    } else {
        out.line(table(&["LOADER", "NEWEST", "AVAILABLE"], &rows));
    }
    Ok(())
}
