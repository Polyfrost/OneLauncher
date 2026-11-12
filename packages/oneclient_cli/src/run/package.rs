use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use clap::{Args, Subcommand, ValueEnum};
use oneclient_common::domain::{ContentType, GameLoader, ProviderId};
use oneclient_content::packages::types::{PackageBody, ProjectDetail};
use oneclient_content::packages::{
    CachedPackageMeta, LinkedArtifactInfo, LiveSync, PackageStore, SearchFilters,
};
use oneclient_core::game::FolderPack;
use oneclient_core::{BrowserPackageUpdate, Cluster, LauncherState};
use serde::{Deserialize, Serialize};

use super::{clusters_changed, find_cluster};
use crate::output::{accent, bold, bytes, count, dim, good, heading, table, warn, when};
use crate::{Kind, Loader, Output, Provider, Side, Sort};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum PackageCommand {
    /// Search Modrinth or CurseForge
    Search(SearchArgs),
    /// List the categories a search can filter by
    Categories {
        #[arg(short, long, value_enum, default_value_t = Provider::Modrinth)]
        provider: Provider,
        #[arg(short, long, value_enum, default_value_t = Kind::Mod)]
        kind: Kind,
    },
    /// Show a package's details
    Info {
        /// Project id or slug
        id: String,
        #[arg(short, long, value_enum, default_value_t = Provider::Modrinth)]
        provider: Provider,
        #[arg(long)]
        json: bool,
        /// Print straight to the terminal
        #[arg(long)]
        no_pager: bool,
    },
    /// List a package's versions
    Versions {
        /// Project id or slug
        id: String,
        #[arg(short, long, value_enum, default_value_t = Provider::Modrinth)]
        provider: Provider,
        /// Only versions that fit this cluster
        #[arg(short, long)]
        cluster: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// List the packages in a cluster
    #[command(visible_alias = "ls")]
    List {
        cluster: String,
        /// Only this kind of package
        #[arg(short, long, value_enum)]
        kind: Option<Kind>,
        #[arg(long)]
        json: bool,
        /// One line per package with its link, to paste somewhere
        #[arg(long, conflicts_with = "json")]
        export: bool,
        #[arg(short, long, value_enum, default_value_t = ListSort::Name)]
        sort: ListSort,
    },
    /// Add a package and its dependencies to a cluster
    #[command(visible_alias = "add")]
    Install {
        cluster: String,
        /// Project id or slug
        id: String,
        #[arg(short, long, value_enum, default_value_t = Provider::Modrinth)]
        provider: Provider,
        /// Version id, the newest that fits the cluster when left out
        #[arg(long = "version")]
        version_id: Option<String>,
        /// Put a data pack into this world
        #[arg(short, long)]
        world: Option<String>,
        /// Install even when the package is known to cause problems or replaces a bundled version
        #[arg(long)]
        force: bool,
        /// The id is a mod id from a crash report, find the Modrinth project that provides it
        #[arg(long, conflicts_with = "version_id")]
        mod_id: bool,
    },
    /// Add package files from disk to a cluster
    Import {
        cluster: String,
        #[arg(required = true)]
        files: Vec<PathBuf>,
        #[arg(short, long, value_enum, default_value_t = Kind::Mod)]
        kind: Kind,
    },
    /// Remove packages from a cluster
    #[command(visible_alias = "rm")]
    Remove {
        cluster: String,
        /// Package names, file names or project ids
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Bring back bundled packages that were removed
    Restore {
        cluster: String,
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Show what changed in a version of a package
    Changelog {
        /// Project id or slug
        id: String,
        #[arg(short, long, value_enum, default_value_t = Provider::Modrinth)]
        provider: Provider,
        /// Version id, the newest when left out
        #[arg(long = "version")]
        version_id: Option<String>,
        /// Print straight to the terminal
        #[arg(long)]
        no_pager: bool,
    },
    /// Turn packages back on
    Enable {
        cluster: String,
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// Turn packages off without removing them
    Disable {
        cluster: String,
        #[arg(required = true)]
        packages: Vec<String>,
        /// Also turn off the packages that require them
        #[arg(long)]
        with_dependents: bool,
        /// Turn them off even when they warn against it, or only them when other packages require them
        #[arg(long)]
        force: bool,
    },
    /// List the updates found for a cluster's packages
    Updates {
        cluster: String,
        /// Ask Modrinth and CurseForge again instead of showing the last check
        #[arg(long)]
        check: bool,
    },
    /// Update packages to their newest version
    Update {
        cluster: String,
        packages: Vec<String>,
        /// Every package with an update, skipped ones included
        #[arg(long, conflicts_with = "packages")]
        all: bool,
    },
    /// Stop offering the update found for a package
    SkipUpdate { cluster: String, package: String },
    /// Print where a package's file is stored
    Path { cluster: String, package: String },
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum ListSort {
    Name,
    /// Largest first
    Size,
    /// Most recently added first
    Added,
    /// Most recently published version first
    Updated,
}

#[derive(Debug, Clone, Args, Serialize, Deserialize)]
pub struct SearchArgs {
    pub query: Vec<String>,
    #[arg(short, long, value_enum, default_value_t = Provider::Modrinth)]
    pub provider: Provider,
    #[arg(short, long, value_enum, default_value_t = Kind::Mod)]
    pub kind: Kind,
    /// Only packages that fit this cluster
    #[arg(short, long)]
    pub cluster: Option<String>,
    /// Only packages for this Minecraft version
    #[arg(short = 'V', long = "version", conflicts_with = "cluster")]
    pub mc_version: Option<String>,
    #[arg(short, long, value_enum, conflicts_with = "cluster")]
    pub loader: Option<Loader>,
    /// Only packages in this category, see `package categories`
    #[arg(long = "category")]
    pub categories: Vec<String>,
    #[arg(short, long, value_enum, default_value_t = Sort::Relevance)]
    pub sort: Sort,
    /// Only mods that run on this side
    #[arg(long, value_enum)]
    pub side: Option<Side>,
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
    /// Which page of results, from 1
    #[arg(long, default_value_t = 1)]
    pub page: usize,
    #[arg(long)]
    pub json: bool,
}

const LISTED_GAME_VERSIONS: usize = 12;

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: PackageCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        PackageCommand::Search(args) => search(state, args, out).await,
        PackageCommand::Info {
            id,
            provider,
            json,
            no_pager,
        } => {
            let project = state
                .services
                .packages
                .get(provider.into())?
                .get_project_with_body(&id, &state.services.content())
                .await?;
            if json {
                out.line(serde_json::to_string_pretty(&project)?);
            } else if no_pager {
                out.line(describe(&project));
            } else {
                out.page(describe(&project));
            }
            Ok(())
        }
        PackageCommand::Versions {
            id,
            provider,
            cluster,
            limit,
        } => {
            let ctx = state.services.content();
            let provider = state.services.packages.get(provider.into())?;
            let project = provider.get_project(&id, &ctx).await?;
            let cluster = match cluster {
                Some(query) => Some(find_cluster(state, &query).await?),
                None => None,
            };
            let page = provider
                .list_versions(
                    &project.id,
                    cluster.as_ref().map(|c| c.mc_version.as_str()),
                    cluster.as_ref().and_then(|c| loader_filter(&project, c)),
                    0,
                    limit,
                    &ctx,
                )
                .await?;
            if page.items.is_empty() {
                out.line(format!("{} has no matching versions", project.name));
                return Ok(());
            }

            let rows: Vec<Vec<String>> = page
                .items
                .iter()
                .map(|version| {
                    vec![
                        bold(&version.version_number),
                        format!("{:?}", version.release_type).to_lowercase(),
                        short_list(&version.game_versions, 4),
                        version
                            .loaders
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", "),
                        when(version.published),
                        dim(&version.version_id),
                    ]
                })
                .collect();
            out.line(table(
                &["VERSION", "TYPE", "MINECRAFT", "LOADERS", "PUBLISHED", "ID"],
                &rows,
            ));
            Ok(())
        }
        PackageCommand::List {
            cluster,
            kind,
            json,
            export,
            sort,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let kind = kind.map(ContentType::from);
            if json {
                let mut packages =
                    PackageStore::list_linked_artifacts(cluster.id, &state.services.content())
                        .await?;
                packages.retain(|p| kind.is_none_or(|kind| p.content_type == kind));
                out.line(serde_json::to_string_pretty(&packages)?);
                return Ok(());
            }
            let mut packages = installed(state, &cluster).await?;
            packages.retain(|(_, p)| kind.is_none_or(|kind| p.content_type == kind));
            let folders = if kind.is_none_or(|kind| kind == ContentType::ResourcePack) {
                oneclient_core::game::list_folder_packs(&cluster.game_dir()?).await
            } else {
                Vec::new()
            };
            if packages.is_empty() && folders.is_empty() {
                out.line(format!("{} has no packages", cluster.name));
                return Ok(());
            }
            let bundled = Bundled::load(state, &cluster).await;

            packages.sort_by(|(a_name, a), (b_name, b)| {
                (a.content_type as u8)
                    .cmp(&(b.content_type as u8))
                    .then_with(|| match sort {
                        ListSort::Name => std::cmp::Ordering::Equal,
                        ListSort::Size => b.size_bytes.cmp(&a.size_bytes),
                        ListSort::Added => b.added_at.cmp(&a.added_at),
                        ListSort::Updated => b.published_at.cmp(&a.published_at),
                    })
                    .then_with(|| a_name.to_lowercase().cmp(&b_name.to_lowercase()))
            });
            if export {
                let ctx = state.services.content();
                let packages: Vec<_> = packages
                    .into_iter()
                    .filter(|(_, p)| !bundled.is_deleted(p))
                    .collect();
                let infos: Vec<_> = packages.iter().map(|(_, p)| p.clone()).collect();
                let meta = package_meta(&ctx, &infos).await;
                let mut lines: Vec<(u8, String, String)> = packages
                    .iter()
                    .map(|(name, package)| {
                        (
                            package.content_type as u8,
                            name.to_lowercase(),
                            export_line(name, package, &bundled, &meta),
                        )
                    })
                    .collect();
                lines.extend(folders.iter().map(|pack| {
                    (
                        ContentType::ResourcePack as u8,
                        pack.name.to_lowercase(),
                        folder_export_line(pack),
                    )
                }));
                lines.sort();
                out.line(format!("{}\n", list_header(&cluster)));
                for (_, _, line) in lines {
                    out.line(line);
                }
                return Ok(());
            }
            let skipped: HashSet<String> =
                oneclient_core::game::shadowed_bundle_mods(state, cluster.id)
                    .await
                    .copies
                    .into_values()
                    .flatten()
                    .collect();
            let dated = |at: &Option<String>| {
                at.as_deref()
                    .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                    .map(|at| when(at.to_utc()))
                    .unwrap_or_default()
            };
            let date_header = match sort {
                ListSort::Added => Some("ADDED"),
                ListSort::Updated => Some("PUBLISHED"),
                _ => None,
            };
            let mut rows: Vec<Vec<String>> = packages
                .iter()
                .map(|(name, package)| {
                    let mut row = vec![
                        if package.enabled {
                            bold(name)
                        } else {
                            dim(name)
                        },
                        package.display_version.clone().unwrap_or_default(),
                        package.content_type.to_string(),
                        package.size_bytes.map(bytes).unwrap_or_default(),
                        if bundled.is_deleted(package) {
                            dim("deleted")
                        } else if skipped.contains(&package.hash) {
                            dim("skipped, another copy loads")
                        } else if package.enabled {
                            good("enabled")
                        } else {
                            warn("disabled")
                        },
                        match bundled.packs.get(&package.hash) {
                            Some(pack) => dim(pack),
                            None if bundled.contains(package) => dim("bundle"),
                            None => package
                                .provider
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| dim("local")),
                        },
                    ];
                    match sort {
                        ListSort::Added => row.push(dated(&package.added_at)),
                        ListSort::Updated => row.push(dated(&package.published_at)),
                        _ => {}
                    }
                    row
                })
                .collect();
            rows.extend(folders.iter().map(|pack| {
                let mut row = vec![
                    if pack.enabled {
                        bold(&pack.name)
                    } else {
                        dim(&pack.name)
                    },
                    dim("folder pack"),
                    ContentType::ResourcePack.to_string(),
                    String::new(),
                    if pack.enabled {
                        good("enabled")
                    } else {
                        warn("disabled")
                    },
                    dim("folder"),
                ];
                row.extend(date_header.map(|_| String::new()));
                row
            }));
            let mut headers = vec!["NAME", "VERSION", "TYPE", "SIZE", "STATE", "SOURCE"];
            headers.extend(date_header);
            out.line(table(&headers, &rows));
            Ok(())
        }
        PackageCommand::Install {
            cluster,
            id,
            provider,
            version_id,
            world,
            force,
            mod_id,
        } => {
            let (id, provider, version_id) = if mod_id {
                let found = find_cluster(state, &cluster).await?;
                let target = oneclient_content::packages::find_mod_for_cluster(
                    state.services.packages.get(ProviderId::Modrinth)?,
                    &id,
                    None,
                    None,
                    found.id,
                    &state.services.content(),
                )
                .await?
                .with_context(|| {
                    format!(
                        "no version of {id} for {}'s Minecraft version and loader is on Modrinth",
                        found.name
                    )
                })?;
                (
                    target.project_id,
                    ProviderId::Modrinth,
                    Some(target.version_id),
                )
            } else {
                (id, provider.into(), version_id)
            };
            install(
                state, &cluster, &id, provider, version_id, world, force, out,
            )
            .await
        }
        PackageCommand::Categories { provider, kind } => {
            let categories = state
                .services
                .packages
                .get(provider.into())?
                .list_categories(kind.into(), &state.services.content())
                .await?;
            if categories.is_empty() {
                out.line("No categories");
            } else {
                out.line(categories.join("\n"));
            }
            Ok(())
        }
        PackageCommand::Import {
            cluster,
            files,
            kind,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let ctx = state.services.content();
            let kind = ContentType::from(kind);
            let files: Vec<(PathBuf, ContentType)> = files
                .into_iter()
                .map(|file| (dunce::canonicalize(&file).unwrap_or(file), kind))
                .collect();
            let report = PackageStore::import_local_files(&files, cluster.id, &ctx).await?;
            let mut deferred = false;
            for row in &report.imported {
                let live = PackageStore::sync_live_content(cluster.id, row, &ctx)
                    .await
                    .unwrap_or(LiveSync::Skipped);
                deferred |= live == LiveSync::Deferred;
            }
            clusters_changed(state);
            for row in &report.imported {
                out.line(format!("Added {}", bold(&row.file_name)));
            }
            for (path, err) in &report.failed {
                out.error(format!(
                    "{} {}: {err}",
                    warn("Could not add"),
                    path.display()
                ));
            }
            if deferred && state.games.is_active(cluster.id) {
                out.line(dim(
                    "Minecraft is running, so they apply at the next launch",
                ));
            }
            if report.imported.is_empty() {
                bail!("nothing was added");
            }
            Ok(())
        }
        PackageCommand::Remove { cluster, packages } => {
            let cluster = find_cluster(state, &cluster).await?;
            let ctx = state.services.content();
            let bundled = Bundled::load(state, &cluster).await;
            for query in packages {
                if let Some(pack) = folder_pack(&cluster, &query).await? {
                    oneclient_core::game::remove_folder_pack(
                        &cluster.game_dir()?,
                        &pack.name,
                        pack.enabled,
                    )
                    .await?;
                    out.line(format!(
                        "Moved the {} folder to the trash",
                        bold(&pack.name)
                    ));
                    continue;
                }
                let (name, package) = find_package(state, &cluster, &query).await?;
                if bundled.contains(&package) {
                    let live = oneclient_core::set_artifact_enabled_to(
                        cluster.id,
                        &package.hash,
                        false,
                        &ctx,
                    )
                    .await?;
                    oneclient_core::on_user_remove_artifact(cluster.id, &package.hash, &ctx)
                        .await?;
                    out.line(format!(
                        "Removed {}, it came with the cluster so `package restore` brings it back",
                        bold(&name)
                    ));
                    if live == LiveSync::Deferred && state.games.is_active(cluster.id) {
                        out.line(dim(
                            "Minecraft is running, so this applies at the next launch",
                        ));
                    }
                    continue;
                }
                let sharing =
                    oneclient_core::clusters_sharing_artifact(cluster.id, &package.hash, &ctx)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or(0);
                oneclient_core::delete_artifact(cluster.id, &package.hash, &ctx).await?;
                out.line(format!("Removed {} from {}", bold(&name), cluster.name));
                if sharing > 1 {
                    out.line(dim(format!(
                        "  {sharing} clusters share this game folder, so it is gone from all of them"
                    )));
                }
            }
            clusters_changed(state);
            Ok(())
        }
        PackageCommand::Restore { cluster, packages } => {
            for package in packages {
                enable(state, &cluster, &package, out).await?;
            }
            Ok(())
        }
        PackageCommand::Changelog {
            id,
            provider,
            version_id,
            no_pager,
        } => {
            let provider_id = ProviderId::from(provider);
            let ctx = state.services.content();
            let provider = state.services.packages.get(provider_id)?;
            let project = provider.get_project(&id, &ctx).await?;
            let version_id = match version_id {
                Some(version_id) => version_id,
                None => {
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
            let version = provider.get_version(&project.id, &version_id, &ctx).await?;
            let mut text = format!(
                "{} {}\n\n{}",
                heading(&project.name),
                bold(&version.version_number),
                match version
                    .changelog
                    .as_deref()
                    .filter(|c| !c.trim().is_empty())
                {
                    Some(changelog) => crate::markdown::render(changelog),
                    None => dim("No changelog for this version"),
                }
            );
            if let Some(url) = oneclient_content::packages::types::version_web_url(
                provider_id,
                &project.id,
                &version.version_id,
                project.web_url().as_deref(),
            ) {
                text.push_str(&format!("\n{}", accent(url)));
            }
            if no_pager {
                out.line(text);
            } else {
                out.page(text);
            }
            Ok(())
        }
        PackageCommand::Enable { cluster, packages } => {
            for package in packages {
                if !toggle_folder_pack(state, &cluster, &package, true, out).await? {
                    enable(state, &cluster, &package, out).await?;
                }
            }
            Ok(())
        }
        PackageCommand::Disable {
            cluster,
            mut packages,
            with_dependents,
            force,
        } => {
            let mut toggled = Vec::new();
            for package in &packages {
                if toggle_folder_pack(state, &cluster, package, false, out).await? {
                    toggled.push(package.clone());
                }
            }
            packages.retain(|package| !toggled.contains(package));
            if packages.is_empty() {
                return Ok(());
            }
            disable(state, &cluster, &packages, with_dependents, force, out).await
        }
        PackageCommand::Updates { cluster, check } => {
            let cluster = find_cluster(state, &cluster).await?;
            let ctx = state.services.content();
            let updates = if check {
                out.line(format!("Checking {} for updates", cluster.name));
                let found =
                    oneclient_core::refresh_browser_package_updates(cluster.id, &ctx).await?;
                clusters_changed(state);
                found.updates
            } else {
                oneclient_core::cached_browser_package_updates(cluster.id, &ctx).await?
            };
            if updates.is_empty() {
                out.line(if check {
                    "Everything is up to date".to_string()
                } else {
                    "No updates found, run with --check to look again".to_string()
                });
                return Ok(());
            }
            let rows: Vec<Vec<String>> = updates
                .iter()
                .map(|update| {
                    vec![
                        bold(&update.display_name),
                        update.installed_version_name.clone(),
                        good(&update.latest_version_name),
                        if update.skipped {
                            dim("skipped")
                        } else {
                            String::new()
                        },
                    ]
                })
                .collect();
            out.line(table(&["PACKAGE", "INSTALLED", "NEWEST", ""], &rows));
            Ok(())
        }
        PackageCommand::Update {
            cluster,
            packages,
            all,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            let ctx = state.services.content();
            let mut updates =
                oneclient_core::cached_browser_package_updates(cluster.id, &ctx).await?;
            if updates.is_empty() {
                updates = oneclient_core::refresh_browser_package_updates(cluster.id, &ctx)
                    .await?
                    .updates;
            }
            let chosen: Vec<BrowserPackageUpdate> = if packages.is_empty() {
                updates.into_iter().filter(|u| all || !u.skipped).collect()
            } else {
                let mut chosen = Vec::new();
                for query in &packages {
                    let (name, package) = find_package(state, &cluster, query).await?;
                    let update = updates
                        .iter()
                        .find(|u| u.hash == package.hash)
                        .with_context(|| {
                            format!("{name} has no update, see `package updates --check`")
                        })?;
                    chosen.push(update.clone());
                }
                chosen
            };
            if chosen.is_empty() {
                out.line("Nothing to update");
                return Ok(());
            }
            let (applied, session_id) =
                oneclient_core::install::apply_package_updates(state, &chosen, "Updating packages")
                    .await;
            super::end_progress(state, session_id);
            clusters_changed(state);
            out.line(format!(
                "{} Updated {applied} of {} packages",
                good("✓"),
                chosen.len()
            ));
            Ok(())
        }
        PackageCommand::SkipUpdate { cluster, package } => {
            let cluster = find_cluster(state, &cluster).await?;
            let (name, package) = find_package(state, &cluster, &package).await?;
            oneclient_core::skip_browser_package_update(
                cluster.id,
                &package.hash,
                &state.services.content(),
            )
            .await?;
            clusters_changed(state);
            out.line(format!("{} will not offer this update again", bold(name)));
            Ok(())
        }
        PackageCommand::Path { cluster, package } => {
            let cluster = find_cluster(state, &cluster).await?;
            let (name, package) = find_package(state, &cluster, &package).await?;
            let artifact = oneclient_db::dao::artifact::get_artifact_by_hash(
                &state.services.db,
                &package.hash,
            )
            .await?
            .with_context(|| format!("{name} has no stored file"))?;
            let path = oneclient_content::packages::store::artifact_absolute_path(&artifact.path)?;
            out.line(path.display().to_string());
            Ok(())
        }
    }
}

struct Bundled {
    ids: HashSet<String>,
    removed: HashSet<String>,
    disabled: HashSet<String>,
    packs: HashMap<String, String>,
}

impl Bundled {
    async fn load(state: &Arc<LauncherState>, cluster: &Cluster) -> Self {
        let ctx = state.services.content();
        let mut ids = HashSet::new();
        if let Ok(bundles) =
            oneclient_core::get_bundles_with_update_status(cluster.id, state.bundles.as_ref(), &ctx)
                .await
        {
            for bundle in bundles {
                for (file, _) in bundle.files {
                    ids.insert(file.kind.metadata_id());
                }
            }
        }
        let (mut removed, mut disabled) = (HashSet::new(), HashSet::new());
        for (_, package_id, kind) in oneclient_core::list_cluster_bundle_overrides(cluster.id, &ctx)
            .await
            .unwrap_or_default()
        {
            match kind.as_str() {
                "removed" => removed.insert(package_id),
                "disabled" => disabled.insert(package_id),
                _ => false,
            };
        }
        let packs = oneclient_core::clusters::modpack_file_sources(state, cluster.id)
            .await
            .unwrap_or_default();
        Self {
            ids,
            removed,
            disabled,
            packs,
        }
    }

    fn package_id(package: &LinkedArtifactInfo) -> &str {
        package.project_id.as_deref().unwrap_or(&package.hash)
    }

    fn contains(&self, package: &LinkedArtifactInfo) -> bool {
        self.ids.contains(Self::package_id(package))
            || self.ids.contains(&package.hash)
            || self.packs.contains_key(&package.hash)
    }

    fn is_deleted(&self, package: &LinkedArtifactInfo) -> bool {
        let id = Self::package_id(package);
        !package.enabled && self.removed.contains(id) && !self.disabled.contains(id)
    }
}

async fn bundled_pin(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    provider: ProviderId,
    project_id: &str,
) -> Option<String> {
    let bundles = oneclient_core::get_bundles_with_update_status(
        cluster.id,
        state.bundles.as_ref(),
        &state.services.content(),
    )
    .await
    .ok()?;
    bundles.into_iter().find_map(|bundle| {
        bundle
            .files
            .into_iter()
            .find_map(|(file, _)| match file.kind {
                oneclient_content::bundles::BundleFileKind::Managed {
                    provider: pinned_provider,
                    project_id: pinned_project,
                    version_id,
                    ..
                } if pinned_provider == provider && pinned_project == project_id => {
                    Some(version_id)
                }
                _ => None,
            })
    })
}

async fn package_meta(
    ctx: &oneclient_content::ContentCtx,
    packages: &[LinkedArtifactInfo],
) -> HashMap<String, CachedPackageMeta> {
    let mut meta = HashMap::new();
    for provider in ProviderId::REMOTE_PROVIDERS {
        let ids: Vec<String> = packages
            .iter()
            .filter(|p| p.provider == Some(*provider))
            .filter_map(|p| p.project_id.clone())
            .collect();
        match oneclient_content::packages::fetch_package_meta(ctx, *provider, &ids).await {
            Ok(metas) => meta.extend(metas),
            Err(err) => tracing::warn!("could not load {provider} package names: {err:#}"),
        }
    }
    meta
}

fn export_line(
    name: &str,
    package: &LinkedArtifactInfo,
    bundled: &Bundled,
    meta: &HashMap<String, CachedPackageMeta>,
) -> String {
    let id = package.project_id.as_deref().unwrap_or_default();
    let url = match package.provider {
        Some(ProviderId::Modrinth) => Some(format!(
            "https://modrinth.com/{}/{id}",
            package.content_type.modrinth_type()
        )),
        Some(ProviderId::CurseForge) => Some(format!("https://www.curseforge.com/projects/{id}")),
        _ => None,
    };
    let version = package
        .display_version
        .clone()
        .filter(|version| !version.is_empty())
        .map(|version| {
            if bundled.contains(package) {
                format!("{version} (bundled)")
            } else {
                version
            }
        });

    let mut segments = vec![name.to_string()];
    segments.extend(url);
    segments.extend(version);
    segments.extend(
        meta.get(id)
            .map(|meta| &meta.author)
            .filter(|author| !author.is_empty())
            .map(|author| format!("by {author}")),
    );
    segments.push(
        if package.enabled {
            "enabled"
        } else {
            "disabled"
        }
        .to_string(),
    );
    format!("- {}", segments.join(" | "))
}

fn folder_export_line(pack: &FolderPack) -> String {
    format!(
        "- {} | Folder pack | {}",
        pack.name,
        if pack.enabled { "enabled" } else { "disabled" }
    )
}

fn list_header(cluster: &Cluster) -> String {
    let loader = if cluster.mc_loader.is_modded() {
        cluster.mc_loader.to_string()
    } else {
        "Vanilla".to_string()
    };
    let loader_version = cluster
        .mc_loader_version
        .as_deref()
        .map_or_else(String::new, |version| format!(" {version}"));
    format!(
        "Minecraft {} · {loader}{loader_version}",
        cluster.mc_version
    )
}

async fn folder_pack(cluster: &Cluster, query: &str) -> anyhow::Result<Option<FolderPack>> {
    let mut packs: Vec<FolderPack> = oneclient_core::game::list_folder_packs(&cluster.game_dir()?)
        .await
        .into_iter()
        .filter(|pack| pack.name == query)
        .collect();
    packs.sort_by_key(|pack| !pack.enabled);
    Ok(packs.into_iter().next())
}

async fn toggle_folder_pack(
    state: &LauncherState,
    query_cluster: &str,
    query: &str,
    enabled: bool,
    out: &Output,
) -> anyhow::Result<bool> {
    let cluster = find_cluster(state, query_cluster).await?;
    let Some(pack) = folder_pack(&cluster, query).await? else {
        return Ok(false);
    };
    if pack.enabled != enabled {
        oneclient_core::game::set_folder_pack_enabled(&cluster.game_dir()?, &pack.name, enabled)
            .await?;
        clusters_changed(state);
    }
    out.line(format!(
        "{} {}",
        if enabled { "Enabled" } else { "Disabled" },
        bold(&pack.name)
    ));
    Ok(true)
}

async fn installed(
    state: &LauncherState,
    cluster: &Cluster,
) -> anyhow::Result<Vec<(String, LinkedArtifactInfo)>> {
    let ctx = state.services.content();
    let packages = PackageStore::list_linked_artifacts(cluster.id, &ctx).await?;
    let meta = package_meta(&ctx, &packages).await;

    Ok(packages
        .into_iter()
        .map(|package| {
            let name = package
                .project_id
                .as_ref()
                .and_then(|id| meta.get(id).map(|meta| meta.name.clone()))
                .or_else(|| package.display_name.clone())
                .unwrap_or_else(|| package.file_name.clone());
            (name, package)
        })
        .collect())
}

fn loader_filter(project: &ProjectDetail, cluster: &Cluster) -> Option<GameLoader> {
    (project.content_type == ContentType::Mod && cluster.mc_loader.is_modded())
        .then_some(cluster.mc_loader)
}

fn short_list(items: &[String], shown: usize) -> String {
    match items.len() {
        n if n <= shown => items.join(", "),
        n => format!(
            "{} {}",
            items[n - shown..].join(", "),
            dim(format!("+{} older", n - shown))
        ),
    }
}

async fn search(state: &Arc<LauncherState>, args: SearchArgs, out: &Output) -> anyhow::Result<()> {
    let content_type = ContentType::from(args.kind);
    if args.side.is_some()
        && (ProviderId::from(args.provider) != ProviderId::Modrinth
            || content_type != ContentType::Mod)
    {
        bail!("--side only filters Modrinth mods");
    }
    let (game_versions, loaders) = match &args.cluster {
        Some(query) => {
            let cluster = find_cluster(state, query).await?;
            if cluster.mc_loader == GameLoader::Ornithe
                && ProviderId::from(args.provider) == ProviderId::CurseForge
                && matches!(content_type, ContentType::Mod | ContentType::Modpack)
            {
                bail!(
                    "CurseForge doesn't have any mods for Ornithe clusters, search Modrinth instead"
                );
            }
            let loader = (content_type == ContentType::Mod && cluster.mc_loader.is_modded())
                .then(|| vec![cluster.mc_loader]);
            (Some(vec![cluster.mc_version]), loader)
        }
        None => (
            args.mc_version.map(|version| vec![version]),
            args.loader.map(|loader| vec![loader.into()]),
        ),
    };

    let query = args.query.join(" ");
    let filters = SearchFilters {
        query: (!query.trim().is_empty()).then_some(query),
        offset: Some(args.page.saturating_sub(1) * args.limit),
        limit: Some(args.limit),
        content_type: Some(content_type),
        game_versions,
        loaders,
        categories: (!args.categories.is_empty()).then_some(args.categories),
        sort: Some(args.sort.into()),
        side: args.side.map(Into::into),
    };
    let page = state
        .services
        .packages
        .get(args.provider.into())?
        .search(&filters, &state.services.content())
        .await?;

    if args.json {
        out.line(serde_json::to_string_pretty(&page.items)?);
        return Ok(());
    }
    if page.items.is_empty() {
        out.line("No results");
        return Ok(());
    }

    let mut lines = Vec::new();
    for project in &page.items {
        lines.push(format!(
            "{} {} {}",
            heading(&project.name),
            dim(format!("by {}", project.author)),
            dim(format!("· {} downloads", count(project.downloads))),
        ));
        if !project.summary.is_empty() {
            lines.push(format!("  {}", project.summary));
        }
        lines.push(format!("  {}", accent(&project.slug)));
        lines.push(String::new());
    }
    lines.push(dim(format!(
        "{} of {} results, details with `oneclient_cli package info <slug>`",
        page.items.len(),
        page.total
    )));
    out.line(lines.join("\n"));
    Ok(())
}

fn describe(project: &ProjectDetail) -> String {
    let field = |name: &str, value: String| format!("  {} {value}", dim(format!("{name:<11}")));
    let mut lines = vec![
        format!(
            "{} {}",
            heading(&project.name),
            dim(format!("{} · {}", project.content_type, project.provider))
        ),
        project.summary.clone(),
        String::new(),
    ];

    let authors = if project.members.is_empty() {
        project.author.clone()
    } else {
        project
            .members
            .iter()
            .map(|member| format!("{} {}", member.name, dim(format!("({})", member.role))))
            .collect::<Vec<_>>()
            .join(", ")
    };
    lines.push(field("Authors", authors));
    lines.push(field("Downloads", count(project.downloads)));
    if let Some(license) = &project.license {
        lines.push(field("License", license.clone()));
    }
    if !project.loaders.is_empty() {
        let loaders: Vec<String> = project.loaders.iter().map(ToString::to_string).collect();
        lines.push(field("Loaders", loaders.join(", ")));
    }
    if !project.game_versions.is_empty() {
        lines.push(field(
            "Minecraft",
            short_list(&project.game_versions, LISTED_GAME_VERSIONS),
        ));
    }
    lines.push(field("Updated", when(project.updated)));
    lines.push(field("Created", when(project.created)));
    lines.push(field(
        "Id",
        format!("{} {}", accent(&project.slug), dim(&project.id)),
    ));
    for (label, url) in &project.links {
        lines.push(field(label, accent(url)));
    }

    lines.push(String::new());
    match &project.body {
        PackageBody::Raw(markdown) => lines.push(crate::markdown::render(markdown)),
        PackageBody::Url(url) => lines.push(format!("Description: {}", accent(url))),
    }
    lines.join("\n")
}

#[allow(clippy::too_many_arguments)]
async fn install(
    state: &Arc<LauncherState>,
    cluster: &str,
    id: &str,
    provider_id: ProviderId,
    version_id: Option<String>,
    world: Option<String>,
    force: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let cluster = find_cluster(state, cluster).await?;
    let ctx = state.services.content();
    let provider = state.services.packages.get(provider_id)?;
    let project = provider.get_project(id, &ctx).await?;
    if project.content_type == ContentType::Modpack {
        bail!(
            "{} is a modpack, install it with `oneclient_cli modpack install {}`",
            project.name,
            project.slug
        );
    }

    let version_id = match version_id {
        Some(version_id) => version_id,
        None => {
            provider
                .list_versions(
                    &project.id,
                    Some(&cluster.mc_version),
                    loader_filter(&project, &cluster),
                    0,
                    1,
                    &ctx,
                )
                .await?
                .items
                .into_iter()
                .next()
                .with_context(|| {
                    format!(
                        "{} has no version for {} {}",
                        project.name, cluster.mc_loader, cluster.mc_version
                    )
                })?
                .version_id
        }
    };

    if !force
        && let Some(pinned) = bundled_pin(state, &cluster, provider_id, &project.id).await
        && pinned != version_id
    {
        bail!(
            "{} comes with this cluster's bundle, pinned to {pinned}. Installing another version overrides the bundle's copy and can break other mods, add --force to install it anyway",
            project.name
        );
    }

    out.line(format!(
        "Installing {} into {}",
        bold(&project.name),
        cluster.name
    ));
    let install = match oneclient_core::install::install_package(
        state,
        provider_id,
        &project.id,
        &version_id,
        cluster.id,
        world,
        force,
    )
    .await
    {
        Ok(install) => install,
        Err(flagged) => bail!(
            "{} is known to cause problems on {}{}, add --force to install it anyway",
            flagged.name,
            flagged.mc_version,
            flagged
                .explanation
                .map(|why| format!(": {why}"))
                .unwrap_or_default()
        ),
    };

    if let Some(session_id) = install.session_id {
        super::end_progress(state, session_id);
    }

    let name = install.result?;
    if let Err(err) =
        oneclient_content::packages::reconcile_duplicate_activity(cluster.id, &ctx).await
    {
        tracing::warn!(
            cluster_id = cluster.id,
            "could not reconcile duplicate packages: {err:#}"
        );
    }
    clusters_changed(state);
    out.line(format!(
        "{} {}",
        good("✓"),
        oneclient_core::install::install_body(
            &name,
            &install.dependencies,
            &install.missing_dependencies,
            install.live_deferred,
        )
    ));
    Ok(())
}

async fn find_package(
    state: &LauncherState,
    cluster: &Cluster,
    query: &str,
) -> anyhow::Result<(String, LinkedArtifactInfo)> {
    let lower = query.to_lowercase();
    let matches: Vec<(String, LinkedArtifactInfo)> = installed(state, cluster)
        .await?
        .into_iter()
        .filter(|(name, p)| {
            name.to_lowercase() == lower
                || p.file_name == query
                || p.cluster_file_name == query
                || p.project_id.as_deref() == Some(query)
                || (query.len() >= 6 && p.hash.starts_with(query))
        })
        .collect();

    match <[(String, LinkedArtifactInfo); 1]>::try_from(matches) {
        Ok([only]) => Ok(only),
        Err(matches) if matches.is_empty() => bail!(
            "{} has no package \"{query}\", see `oneclient_cli package list`",
            cluster.name
        ),
        Err(matches) => bail!(
            "\"{query}\" matches {} packages, use a file name instead: {}",
            matches.len(),
            matches
                .iter()
                .map(|(_, p)| p.file_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

async fn enable(
    state: &Arc<LauncherState>,
    cluster: &str,
    package: &str,
    out: &Output,
) -> anyhow::Result<()> {
    let cluster = find_cluster(state, cluster).await?;
    let (name, package) = find_package(state, &cluster, package).await?;
    let live = oneclient_core::set_artifact_enabled_to(
        cluster.id,
        &package.hash,
        true,
        &state.services.content(),
    )
    .await?;
    clusters_changed(state);

    out.line(format!("Enabled {}", bold(name)));
    if live == LiveSync::Deferred && state.games.is_active(cluster.id) {
        out.line(dim(
            "Minecraft is running, so this applies at the next launch",
        ));
    }
    Ok(())
}

async fn disable(
    state: &Arc<LauncherState>,
    cluster: &str,
    packages: &[String],
    with_dependents: bool,
    force: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let cluster = find_cluster(state, cluster).await?;
    let mut found = Vec::new();
    for package in packages {
        found.push(find_package(state, &cluster, package).await?);
    }
    let names: Vec<String> = found.iter().map(|(name, _)| name.clone()).collect();
    let roots: Vec<_> = found
        .iter()
        .map(|(_, package)| oneclient_core::DisableRoot::Artifact(package.hash.clone()))
        .collect();

    let warned = disable_warnings(state, &cluster, &found).await;
    if !warned.is_empty() && !force {
        for (_, name, body) in &warned {
            out.line(format!("{}\n{body}", bold(name)));
        }
        bail!("add --force to turn it off anyway");
    }

    let ctx = state.services.content();
    let plan = oneclient_core::plan_disable(cluster.id, &roots, &ctx).await?;
    let required: Vec<String> = plan.impact.required.into_iter().map(|d| d.name).collect();
    let alone = !required.is_empty() && !with_dependents;
    if alone && !force {
        let shown = match required.len() {
            n if n <= 5 => required.join(", "),
            n => format!("{} and {} more", required[..4].join(", "), n - 4),
        };
        bail!(
            "{} {} needed by {shown}, add --with-dependents to turn {} off too or --force to leave {} on",
            names.join(", "),
            if names.len() == 1 { "is" } else { "are" },
            if required.len() == 1 { "it" } else { "them" },
            if required.len() == 1 { "it" } else { "them" }
        );
    }
    if !plan.impact.optional.is_empty() {
        let optional: Vec<String> = plan.impact.optional.into_iter().map(|d| d.name).collect();
        out.line(dim(format!(
            "{} optionally {} on it and stays enabled",
            optional.join(", "),
            if optional.len() == 1 {
                "depends"
            } else {
                "depend"
            }
        )));
    }

    let hashes = if alone {
        &plan.hashes[..roots.len()]
    } else {
        &plan.hashes[..]
    };
    oneclient_core::apply_disable(cluster.id, hashes, &[], &ctx).await?;
    clusters_changed(state);
    acknowledge_disable_warnings(state, &warned).await;

    let dependents: &[String] = if alone { &[] } else { &required };
    for name in names.iter().chain(dependents) {
        out.line(format!("Disabled {}", bold(name)));
    }
    Ok(())
}

const POLYPLUS: &str = "Iw9mZi4a";
const POLYPLUS_DISABLE_BODY: &str = "Poly+ powers your cosmetics and playtime tracking. Turning it off disables them in game, and it stays off through future bundle updates until you turn it back on.";

async fn disable_warnings(
    state: &Arc<LauncherState>,
    cluster: &Cluster,
    found: &[(String, LinkedArtifactInfo)],
) -> Vec<(String, String, String)> {
    let acknowledged = state.settings.read().acknowledged_disable_warnings.clone();
    let candidates: Vec<(&String, &str)> = found
        .iter()
        .filter(|(_, p)| p.provider == Some(ProviderId::Modrinth))
        .map(|(name, p)| (name, Bundled::package_id(p)))
        .filter(|(_, id)| !acknowledged.iter().any(|a| a == id))
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    let bundled = Bundled::load(state, cluster).await;
    let warnings = if candidates.iter().any(|(_, id)| bundled.ids.contains(*id)) {
        oneclient_core::fetch_disable_warnings(&state.services.requester)
            .await
            .ok()
    } else {
        None
    };
    candidates
        .into_iter()
        .filter_map(|(name, id)| {
            let body = match &warnings {
                Some(warnings) if bundled.ids.contains(id) => warnings.body_for(id)?.to_string(),
                _ if id == POLYPLUS => POLYPLUS_DISABLE_BODY.to_string(),
                _ => return None,
            };
            Some((id.to_string(), name.clone(), body))
        })
        .collect()
}

async fn acknowledge_disable_warnings(
    state: &Arc<LauncherState>,
    warned: &[(String, String, String)],
) {
    if warned.is_empty() {
        return;
    }
    let updated = {
        let mut settings = state.settings.write();
        for (id, _, _) in warned {
            if !settings.acknowledged_disable_warnings.contains(id) {
                settings.acknowledged_disable_warnings.push(id.clone());
            }
        }
        settings.clone()
    };
    if let Err(err) =
        oneclient_core::settings::store::save_settings_and_apply(&state.services, &updated).await
    {
        tracing::warn!(%err, "could not remember the acknowledged disable warnings");
    }
}
