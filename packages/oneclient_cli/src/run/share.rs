use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, bail};
use clap::{Subcommand, ValueEnum};
use oneclient_core::clusters::{ExportMode, ModpackSource, SettingsChoice};
use oneclient_core::sharing::{
    self, LinkedPush, PublishedShare, ShareCode, ShareRole, ShareStore, ShareUpdate,
};
use oneclient_core::sync::{self, CloudSync, Resolution, SyncAccount, SyncOutcome, SyncStatus};
use oneclient_core::{Cluster, LauncherState};
use oneclient_events::{Choice, GroupedProgressSession, Prompt};
use serde::{Deserialize, Serialize};

use super::cluster::{ExportArgs, claim_export, export_report, idle_cluster};
use super::modpack::Job;
use super::{clusters_changed, find_cluster};
use crate::Output;
use crate::output::{accent, bad, bold, dim, good, heading, table, warn, when};

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum ShareCommand {
    /// Share a cluster and print its code, or send your changes to its share
    ///
    /// Without export flags an update keeps what the share included last time.
    Publish {
        cluster: String,
        #[command(flatten)]
        options: ExportArgs,
        /// Only send the settings files, keeping the shared mods as they are
        #[arg(long)]
        settings_only: bool,
    },
    /// Show a cluster's share code, or the share it was imported from
    Status { cluster: String },
    /// Take a cluster's share down so its code stops working
    Stop { cluster: String },
    /// Import a shared cluster by its code or link
    Import {
        code: String,
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
    /// Update a cluster from the share it was imported from
    Pull { cluster: String },
    /// Stop a cluster following the share it was imported from
    Unlink { cluster: String },
    /// List the players who can update a cluster's share
    Collaborators { cluster: String },
    /// Let a player update a cluster's share
    AddCollaborator { cluster: String, username: String },
    /// Stop a player updating a cluster's share
    RemoveCollaborator { cluster: String, player: String },
    /// Keep clusters the same on every computer you sign in on
    #[command(subcommand)]
    Cloud(CloudCommand),
}

#[derive(Debug, Clone, Subcommand, Serialize, Deserialize)]
pub enum CloudCommand {
    /// List the clusters synced to the cloud
    #[command(visible_alias = "ls")]
    List,
    /// Start syncing a cluster to the cloud
    Enable { cluster: String },
    /// Stop syncing a cluster and remove it from the cloud, copies elsewhere stay
    Disable { cluster: String },
    /// Sync one cluster now, or all of them and bring in new ones from your other computers
    Sync { cluster: Option<String> },
    /// Pick the copy to keep after a cluster changed here and on another computer
    Resolve {
        cluster: String,
        #[arg(value_enum)]
        keep: Keep,
    },
    /// Show or set the settings files that stay on this computer instead of syncing
    LocalFiles {
        cluster: String,
        /// Replaces the list, see `cluster setting-files`
        paths: Vec<String>,
        /// Sync every settings file again
        #[arg(long, conflicts_with = "paths")]
        clear: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum Keep {
    /// This computer's copy
    Local,
    /// The cloud's copy
    Cloud,
    /// Both, the cloud copy becomes its own cluster
    Both,
}

impl From<Keep> for Resolution {
    fn from(keep: Keep) -> Self {
        match keep {
            Keep::Local => Self::KeepLocal,
            Keep::Cloud => Self::KeepCloud,
            Keep::Both => Self::KeepBoth,
        }
    }
}

pub(super) async fn run(
    state: &Arc<LauncherState>,
    command: ShareCommand,
    out: &Output,
) -> anyhow::Result<()> {
    let store = sharing::default_store(state.auth.clone());
    match command {
        ShareCommand::Publish {
            cluster,
            options,
            settings_only,
        } => {
            publish(
                state,
                store.as_ref(),
                &cluster,
                &options,
                settings_only,
                out,
            )
            .await
        }
        ShareCommand::Status { cluster } => {
            status(
                state,
                store.as_ref(),
                &find_cluster(state, &cluster).await?,
                out,
            )
            .await
        }
        ShareCommand::Stop { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let ledger = sharing::default_ledger()?;
            match sharing::stop_sharing(cluster.id, store.as_ref(), &ledger).await? {
                Some(code) => out.line(format!(
                    "Stopped sharing {}, {} no longer works",
                    bold(&cluster.name),
                    code.pretty()
                )),
                None => bail!("{} is not shared", cluster.name),
            }
            Ok(())
        }
        ShareCommand::Import {
            code,
            name,
            scan,
            force,
        } => {
            let fetched = sharing::fetch_share(store.as_ref(), &code).await?;
            let cluster_id = super::modpack::install(
                state,
                ModpackSource::File(fetched.path.clone()),
                name,
                scan,
                force,
                out,
            )
            .await?;
            let links = sharing::default_linked_ledger()?;
            sharing::link_import(
                state,
                &links,
                cluster_id,
                &fetched.code,
                &fetched.remote,
                &fetched.path,
            )
            .await?;
            out.line(dim(format!(
                "It follows {}, bring in its updates with `oneclient_cli share pull {cluster_id}`",
                fetched.code.pretty()
            )));
            Ok(())
        }
        ShareCommand::Pull { cluster } => pull(state, store.as_ref(), &cluster, out).await,
        ShareCommand::Unlink { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let links = sharing::default_linked_ledger()?;
            if !sharing::unlink(&links, cluster.id).await? {
                bail!("{} was not imported from a share code", cluster.name);
            }
            out.line(format!(
                "{} no longer follows its share code",
                bold(&cluster.name)
            ));
            Ok(())
        }
        ShareCommand::Collaborators { cluster } => {
            let code = owned_code(state, store.as_ref(), &cluster).await?;
            let people = store.collaborators(&code).await?;
            if people.is_empty() {
                out.line("Only you can update this share");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = people
                .into_iter()
                .map(|person| vec![bold(person.username.unwrap_or_default()), dim(person.uuid)])
                .collect();
            out.line(table(&["PLAYER", "UUID"], &rows));
            Ok(())
        }
        ShareCommand::AddCollaborator { cluster, username } => {
            let code = owned_code(state, store.as_ref(), &cluster).await?;
            let person = store.add_collaborator(&code, username.trim()).await?;
            out.line(format!(
                "{} can now update {}",
                bold(person.username.as_deref().unwrap_or(username.trim())),
                code.pretty()
            ));
            Ok(())
        }
        ShareCommand::RemoveCollaborator { cluster, player } => {
            let code = owned_code(state, store.as_ref(), &cluster).await?;
            let person = store
                .collaborators(&code)
                .await?
                .into_iter()
                .find(|person| {
                    person.uuid.to_string() == player
                        || person
                            .username
                            .as_deref()
                            .is_some_and(|name| name.eq_ignore_ascii_case(player.trim()))
                })
                .with_context(|| format!("{player} cannot update {}", code.pretty()))?;
            store.remove_collaborator(&code, person.uuid).await?;
            out.line(format!(
                "{} can no longer update {}",
                bold(person.username.as_deref().unwrap_or(&player)),
                code.pretty()
            ));
            Ok(())
        }
        ShareCommand::Cloud(command) => cloud(state, command, out).await,
    }
}

async fn publish(
    state: &Arc<LauncherState>,
    store: &dyn ShareStore,
    query: &str,
    options: &ExportArgs,
    settings_only: bool,
    out: &Output,
) -> anyhow::Result<()> {
    let cluster = idle_cluster(state, query).await?;
    let ledger = sharing::default_ledger()?;
    let links = sharing::default_linked_ledger()?;
    let owned = ledger.find(cluster.id, store.name()).await?;
    let collaborating = match (&owned, links.find(cluster.id).await?) {
        (None, Some(link)) => store
            .describe(&link.code)
            .await
            .is_ok_and(|remote| remote.role == ShareRole::Collaborator),
        _ => false,
    };
    let previous = owned
        .as_ref()
        .and_then(|entry| entry.push.as_ref())
        .map(|push| push.options.clone());
    let export = match previous {
        Some(previous) if options.is_empty() => previous,
        _ => options.options(state, &cluster).await?,
    };
    if settings_only && !export.settings.includes_any() {
        bail!("pick the settings files to send with --settings or --setting");
    }

    let _slot = claim_export(&cluster)?;
    let session =
        GroupedProgressSession::start(&state.services.events, format!("Sharing {}", cluster.name));
    let shared = match (collaborating, settings_only) {
        (true, true) => {
            sharing::push_linked(
                state,
                cluster.id,
                LinkedPush::SettingsOnly(export.settings),
                store,
                &links,
                Some(&session),
            )
            .await
        }
        (true, false) => {
            sharing::push_linked(
                state,
                cluster.id,
                LinkedPush::Everything(export),
                store,
                &links,
                Some(&session),
            )
            .await
        }
        (false, true) => {
            sharing::share_settings_only(
                state,
                cluster.id,
                export.settings,
                store,
                &ledger,
                Some(&session),
            )
            .await
        }
        (false, false) => {
            sharing::share_cluster(state, cluster.id, export, store, &ledger, Some(&session)).await
        }
    };
    if shared.is_ok() {
        session.finish();
    } else {
        session.abort();
    }
    let shared = shared?;

    out.line(format!(
        "{} {} {}",
        good("✓"),
        if shared.updated {
            "Updated the share of"
        } else {
            "Shared"
        },
        bold(&cluster.name)
    ));
    print_share(&shared.published, out);
    export_report(&shared.report, out);
    Ok(())
}

fn print_share(share: &PublishedShare, out: &Output) {
    out.line(format!(
        "  Code {}  {}",
        heading(share.code.pretty()),
        accent(&share.link)
    ));
    if let Some(expires) = share.expires_at {
        out.line(dim(format!("  Expires {}", when(expires))));
    }
}

fn settings_label(settings: &SettingsChoice) -> String {
    match settings {
        SettingsChoice::None => "no settings files".to_string(),
        SettingsChoice::All => "every settings file".to_string(),
        SettingsChoice::Only(paths) => paths.iter().cloned().collect::<Vec<_>>().join(", "),
        SettingsChoice::AllExcept(paths) => format!(
            "every settings file but {}",
            paths.iter().cloned().collect::<Vec<_>>().join(", ")
        ),
    }
}

async fn status(
    state: &Arc<LauncherState>,
    store: &dyn ShareStore,
    cluster: &Cluster,
    out: &Output,
) -> anyhow::Result<()> {
    let ledger = sharing::default_ledger()?;
    let links = sharing::default_linked_ledger()?;
    let owned = ledger.find(cluster.id, store.name()).await?;
    let link = links.find(cluster.id).await?;
    if owned.is_none() && link.is_none() {
        out.line(format!(
            "{} is not shared, share it with `oneclient_cli share publish {}`",
            cluster.name, cluster.id
        ));
        return Ok(());
    }

    let link = link.filter(|link| {
        owned
            .as_ref()
            .is_none_or(|entry| entry.share.code != link.code)
    });

    if let Some(entry) = owned {
        out.line(format!("{} is shared", bold(&cluster.name)));
        print_share(&entry.share, out);
        match sharing::owner_status(state, cluster.id, store, &ledger).await {
            Ok(Some(status)) => {
                out.line(dim(format!(
                    "  {} export with {}",
                    match status.options.mode {
                        ExportMode::OneClient => "OneClient",
                        ExportMode::Compatibility => "Compatible",
                    },
                    settings_label(&status.options.settings)
                )));
                if status.behind {
                    out.line(warn(
                        "  Someone updated the share since, bring it in with `oneclient_cli share pull` first",
                    ));
                }
                match (status.content_changed, status.settings_changed) {
                    (false, false) => out.line(good("  Up to date")),
                    (true, _) => out.line(warn(
                        "  The cluster changed since it was shared, send it with `oneclient_cli share publish`",
                    )),
                    (false, true) => out.line(warn(
                        "  Its settings changed since it was shared, send them with `oneclient_cli share publish --settings-only`",
                    )),
                }
            }
            Ok(None) => {}
            Err(err) => out.line(dim(format!("  Could not check it for changes: {err:#}"))),
        }
    }

    if let Some(link) = link {
        out.line(format!(
            "{} follows {}{}",
            bold(&cluster.name),
            heading(link.code.pretty()),
            dim(format!(" (revision {})", link.revision))
        ));
        match sharing::check_update(store, &links, cluster.id).await {
            Ok(Some(ShareUpdate::Available { changes, .. })) => {
                out.line(warn(format!(
                    "  An update is out, bring it in with `oneclient_cli share pull {}`",
                    cluster.id
                )));
                for line in changes.describe() {
                    out.line(dim(format!("  {line}")));
                }
            }
            Ok(Some(ShareUpdate::Ended { .. })) => {
                out.line(bad("  The share was taken down by its owner"));
            }
            Ok(_) => out.line(good("  Up to date")),
            Err(err) => out.line(dim(format!("  Could not check for updates: {err:#}"))),
        }
    }
    Ok(())
}

async fn pull(
    state: &Arc<LauncherState>,
    store: &dyn ShareStore,
    query: &str,
    out: &Output,
) -> anyhow::Result<()> {
    let cluster = idle_cluster(state, query).await?;
    let links = sharing::default_linked_ledger()?;
    match sharing::check_update(store, &links, cluster.id).await? {
        None => bail!("{} was not imported from a share code", cluster.name),
        Some(ShareUpdate::Current) => {
            out.line(format!("{} is up to date", cluster.name));
            Ok(())
        }
        Some(ShareUpdate::Ended { link }) => {
            sharing::unlink(&links, link.cluster_id).await?;
            out.line(format!(
                "The share {} was taken down, {} keeps what it has and no longer follows it",
                link.code.pretty(),
                bold(&cluster.name)
            ));
            Ok(())
        }
        Some(ShareUpdate::Available { changes, .. }) => {
            for line in changes.describe() {
                out.line(dim(line));
            }
            let _job = Job::claim(state, Some(&cluster))?;
            let session = GroupedProgressSession::start(
                &state.services.events,
                format!("Updating {}", cluster.name),
            );
            let applied = sharing::apply_update(state, store, &links, cluster.id).await;
            if applied.is_ok() {
                session.finish();
            } else {
                session.abort();
            }
            clusters_changed(state);
            let applied = applied?;
            out.line(format!("{} Updated {}", good("✓"), bold(&cluster.name)));
            if applied != cluster.id {
                out.line(dim(format!(
                    "The share moved to another Minecraft version, so it was set up as cluster {applied} and {} was kept",
                    cluster.name
                )));
            }
            out.line(dim("Mods you added yourself were kept"));
            Ok(())
        }
    }
}

async fn owned_code(
    state: &LauncherState,
    store: &dyn ShareStore,
    query: &str,
) -> anyhow::Result<ShareCode> {
    let cluster = find_cluster(state, query).await?;
    let ledger = sharing::default_ledger()?;
    Ok(ledger
        .find(cluster.id, store.name())
        .await?
        .with_context(|| {
            format!(
                "{} is not shared by you, share it with `oneclient_cli share publish {}`",
                cluster.name, cluster.id
            )
        })?
        .share
        .code)
}

async fn cloud(
    state: &Arc<LauncherState>,
    command: CloudCommand,
    out: &Output,
) -> anyhow::Result<()> {
    let engine = sync::engine(state.auth.clone())
        .context("cloud sync is unavailable, its folder could not be set up")?;
    let account = sync::current_account(state).await?;
    let result = cloud_command(state, &engine, &account, command, out).await;
    clusters_changed(state);
    result
}

async fn cloud_command(
    state: &Arc<LauncherState>,
    engine: &CloudSync,
    account: &SyncAccount,
    command: CloudCommand,
    out: &Output,
) -> anyhow::Result<()> {
    match command {
        CloudCommand::List => {
            let mut rows = Vec::new();
            for (cluster_id, status) in engine.statuses(account).await? {
                let name = state
                    .clusters
                    .get(cluster_id)
                    .await
                    .map_or_else(|_| cluster_id.to_string(), |cluster| cluster.name);
                let label = match &status {
                    SyncStatus::Synced => good(status.label()),
                    SyncStatus::Error { message } => bad(format!("{}: {message}", status.label())),
                    SyncStatus::Conflict { .. } => warn(status.label()),
                    _ => status.label().to_string(),
                };
                rows.push(vec![cluster_id.to_string(), bold(name), label]);
            }
            if rows.is_empty() {
                out.line(
                    "No clusters sync to the cloud, start with `oneclient_cli share cloud enable <cluster>`",
                );
                return Ok(());
            }
            rows.sort();
            out.line(table(&["ID", "CLUSTER", "STATUS"], &rows));
            Ok(())
        }
        CloudCommand::Enable { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            let outcome = engine.enable(state, account, cluster.id).await?;
            report(state, &cluster, outcome, out).await;
            Ok(())
        }
        CloudCommand::Disable { cluster } => {
            let cluster = find_cluster(state, &cluster).await?;
            engine.disable(account, cluster.id).await?;
            out.line(format!(
                "{} no longer syncs, copies on your other computers stay",
                bold(&cluster.name)
            ));
            Ok(())
        }
        CloudCommand::Sync {
            cluster: Some(cluster),
        } => {
            let cluster = idle_cluster(state, &cluster).await?;
            let outcome = engine.sync_cluster(state, account, cluster.id).await?;
            report(state, &cluster, outcome, out).await;
            Ok(())
        }
        CloudCommand::Sync { cluster: None } => {
            let report = engine.sync_all(state, account).await?;
            let names = async |ids: &[i64]| {
                let mut names = Vec::new();
                for id in ids {
                    if let Ok(cluster) = state.clusters.get(*id).await {
                        names.push(cluster.name);
                    }
                }
                names.join(", ")
            };
            let lines = [
                (
                    "Added from your other computers",
                    names(&report.created).await,
                ),
                ("Sent", names(&report.pushed).await),
                ("Brought in", names(&report.pulled).await),
                (
                    "Removed from the cloud on another computer, the copy here is kept",
                    names(&report.removed).await,
                ),
            ];
            let mut any = false;
            for (label, names) in lines {
                if !names.is_empty() {
                    any = true;
                    out.line(format!("{label}: {names}"));
                }
            }
            for (old, new) in &report.replaced {
                any = true;
                out.line(format!(
                    "{} uses another Minecraft version on your other computer, so it was set up as a new cluster and {} was kept",
                    names(&[*new]).await,
                    names(&[*old]).await
                ));
            }
            let conflicts = names(&report.conflicts).await;
            if !conflicts.is_empty() {
                any = true;
                out.error(warn(format!(
                    "Changed here and on another computer, pick a copy with `oneclient_cli share cloud resolve`: {conflicts}"
                )));
            }
            for (_, message) in &report.failed {
                any = true;
                out.error(bad(message));
            }
            if !any {
                out.line("Everything is in sync");
            }
            Ok(())
        }
        CloudCommand::Resolve { cluster, keep } => {
            let cluster = find_cluster(state, &cluster).await?;
            if state.games.is_active(cluster.id) {
                bail!("{} is running, close the game first", cluster.name);
            }
            if !matches!(
                engine.statuses(account).await?.get(&cluster.id),
                Some(SyncStatus::Conflict { .. })
            ) {
                bail!("{} has no sync conflict to resolve", cluster.name);
            }
            let outcome = engine
                .resolve(state, account, cluster.id, keep.into())
                .await?;
            report(state, &cluster, outcome, out).await;
            Ok(())
        }
        CloudCommand::LocalFiles {
            cluster,
            paths,
            clear,
        } => {
            let cluster = find_cluster(state, &cluster).await?;
            if paths.is_empty() && !clear {
                let left_out = engine.left_out(cluster.id).await?;
                if left_out.is_empty() {
                    out.line(format!("Every settings file of {} syncs", cluster.name));
                }
                for path in left_out {
                    out.line(path);
                }
                return Ok(());
            }
            let known: Vec<String> =
                oneclient_core::clusters::list_setting_files(state, cluster.id)
                    .await?
                    .into_iter()
                    .map(|file| file.path)
                    .collect();
            if let Some(unknown) = paths.iter().find(|path| !known.contains(path)) {
                bail!(
                    "{} has no settings file {unknown}, see `oneclient_cli cluster setting-files {}`",
                    cluster.name,
                    cluster.id
                );
            }
            let left_out: BTreeSet<String> = paths.into_iter().collect();
            let outcome = engine
                .set_left_out(state, account, cluster.id, left_out)
                .await?;
            out.line(format!(
                "Saved which settings files of {} stay on this computer",
                bold(&cluster.name)
            ));
            if outcome != SyncOutcome::Unchanged {
                report(state, &cluster, outcome, out).await;
            }
            Ok(())
        }
    }
}

async fn report(state: &LauncherState, cluster: &Cluster, outcome: SyncOutcome, out: &Output) {
    let name_of = async |id: i64| {
        state
            .clusters
            .get(id)
            .await
            .map_or_else(|_| id.to_string(), |cluster| cluster.name)
    };
    match outcome {
        SyncOutcome::Unchanged => out.line(format!("{} is in sync", cluster.name)),
        SyncOutcome::Pushed { .. } => out.line(format!(
            "{} Sent {} to the cloud",
            good("✓"),
            bold(&cluster.name)
        )),
        SyncOutcome::Pulled { .. } => out.line(format!(
            "{} Brought in the changes to {} from your other computers",
            good("✓"),
            bold(&cluster.name)
        )),
        SyncOutcome::Replaced {
            old_cluster_id,
            cluster_id,
        } => out.line(format!(
            "{} uses another Minecraft version on your other computer, so it was set up as a new cluster. {} is kept and no longer syncs",
            bold(name_of(cluster_id).await),
            name_of(old_cluster_id).await
        )),
        SyncOutcome::Copied { copy_cluster_id } => out.line(format!(
            "{} Kept both, the cloud copy is now {}",
            good("✓"),
            bold(name_of(copy_cluster_id).await)
        )),
        SyncOutcome::Conflict { .. } => out.error(warn(format!(
            "{} changed here and on another computer, pick a copy with `oneclient_cli share cloud resolve {} local|cloud|both`",
            cluster.name, cluster.id
        ))),
        SyncOutcome::RemovedFromCloud => out.line(format!(
            "{} was removed from the cloud on another computer, the copy here is kept",
            cluster.name
        )),
        SyncOutcome::Deferred => out.line(format!(
            "{} is busy, it syncs once it is free",
            cluster.name
        )),
    }
}

const UPDATE_CHECK_LIMIT: Duration = Duration::from_secs(6);
const SYNC_LIMIT: Duration = Duration::from_secs(90);

#[derive(Clone, Copy)]
enum UpdateAnswer {
    Update,
    Unlink,
}

pub(super) async fn before_launch(state: &Arc<LauncherState>, cluster: &Cluster, out: &Output) {
    cloud_sync_synced(state, cluster, out).await;
    share_update(state, cluster, out).await;
}

pub(super) async fn cloud_sync_synced(state: &Arc<LauncherState>, cluster: &Cluster, out: &Output) {
    let Some(engine) = sync::engine(state.auth.clone()) else {
        return;
    };
    let Ok(account) = sync::current_account(state).await else {
        return;
    };
    if !matches!(
        engine.ledger().find(account.id, cluster.id).await,
        Ok(Some(_))
    ) {
        return;
    }
    match tokio::time::timeout(SYNC_LIMIT, engine.sync_cluster(state, &account, cluster.id)).await {
        Ok(Ok(SyncOutcome::Unchanged)) => {}
        Ok(Ok(outcome)) => {
            clusters_changed(state);
            report(state, cluster, outcome, out).await;
        }
        Ok(Err(err)) => out.error(warn(format!("Cloud sync failed: {err:#}"))),
        Err(_) => out.error(warn("Cloud sync took too long, using this computer's copy")),
    }
}

async fn share_update(state: &Arc<LauncherState>, cluster: &Cluster, out: &Output) {
    let store = sharing::default_store(state.auth.clone());
    let Ok(links) = sharing::default_linked_ledger() else {
        return;
    };
    let update = match tokio::time::timeout(
        UPDATE_CHECK_LIMIT,
        sharing::check_update(store.as_ref(), &links, cluster.id),
    )
    .await
    {
        Ok(Ok(Some(update))) => update,
        Ok(Ok(None)) | Err(_) => return,
        Ok(Err(err)) => {
            tracing::info!(cluster_id = cluster.id, error = %err, "could not check the share code for updates; launching as is");
            return;
        }
    };
    let (link, changes) = match update {
        ShareUpdate::Current => return,
        ShareUpdate::Ended { link } => {
            if sharing::unlink(&links, link.cluster_id).await.is_ok() {
                out.line(dim(format!(
                    "The share {} was taken down, {} keeps what it has",
                    link.code.pretty(),
                    cluster.name
                )));
            }
            return;
        }
        ShareUpdate::Available { link, changes, .. } => (link, changes),
    };

    let mut body = format!(
        "{} follows {}, which was updated.",
        cluster.name,
        link.code.pretty()
    );
    for line in changes.describe() {
        body.push(' ');
        body.push_str(&line);
    }
    body.push_str(" Mods you added yourself stay.");
    let prompt = Prompt::new(format!("Update {}?", cluster.name), body)
        .option(
            Choice::primary("share.update", "Update"),
            UpdateAnswer::Update,
        )
        .option(
            Choice::new("share.unlink", "Stop following it"),
            UpdateAnswer::Unlink,
        )
        .dismiss("Not now");
    match state.services.events.ask(prompt).await {
        Ok(Some(chosen)) => match chosen.value {
            UpdateAnswer::Unlink => {
                if let Err(err) = sharing::unlink(&links, cluster.id).await {
                    tracing::warn!(cluster_id = cluster.id, error = %err, "could not stop share updates");
                }
            }
            UpdateAnswer::Update => {
                let session = GroupedProgressSession::start(
                    &state.services.events,
                    format!("Updating {}", cluster.name),
                );
                let applied =
                    sharing::apply_update(state, store.as_ref(), &links, cluster.id).await;
                if applied.is_ok() {
                    session.finish();
                } else {
                    session.abort();
                }
                clusters_changed(state);
                match applied {
                    Ok(applied) if applied != cluster.id => out.line(dim(format!(
                        "The share moved to another Minecraft version, so it was set up as cluster {applied}"
                    ))),
                    Ok(_) => out.line(format!("{} Updated {}", good("✓"), bold(&cluster.name))),
                    Err(err) => out.error(warn(format!(
                        "Could not update from the share, launching as is: {err:#}"
                    ))),
                }
            }
        },
        Ok(None) => {}
        Err(err) => tracing::warn!(
            cluster_id = cluster.id,
            "share update prompt failed: {err:#}"
        ),
    }
}
