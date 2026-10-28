use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use freya::prelude::spawn_forever;
use oneclient_cluster::ClusterManager;
use oneclient_content::modpacks::{SHARE_EXTENSION, ShareExportReport};
use oneclient_core::LauncherState;
use oneclient_core::clusters::{
    ExportOptions, ModpackSource, SettingsChoice, export_cluster_with, list_setting_files,
};
use oneclient_core::sharing::{
    self, Collaborator, LinkedPush, PublishedShare, RemoteShare, ShareCode, ShareRole, ShareUpdate,
    SharedInstance,
};
use oneclient_db::models::ClusterId;
use oneclient_events::{Choice, GroupedProgressSession, Prompt};

use super::Actions;
use crate::components::IconType;
use crate::launcher::{self, off_ui};
use crate::notifications::{SharedPreview, SharedPreviewFile};
use crate::state::{AppChannel, OwnerCheck, OwnerShare, ShareProgress, ShareReady};

const UPDATE_CHECK_LIMIT: Duration = Duration::from_secs(6);
const MAX_PREVIEW_FILES: usize = 200;

static PENDING_LINKS: Mutex<Option<HashMap<PathBuf, (ShareCode, RemoteShare)>>> = Mutex::new(None);

fn pending_links<R>(
    change: impl FnOnce(&mut HashMap<PathBuf, (ShareCode, RemoteShare)>) -> R,
) -> R {
    let mut guard = PENDING_LINKS.lock().unwrap_or_else(PoisonError::into_inner);
    change(guard.get_or_insert_with(HashMap::new))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UpdateAnswer {
    Update,
    Skip,
    Unlink,
}

const MAX_LISTED: usize = 3;

const NOT_EXPORTING: i64 = -1;

static EXPORTING: AtomicI64 = AtomicI64::new(NOT_EXPORTING);

struct ExportSlot;

impl ExportSlot {
    fn claim(cluster_id: ClusterId) -> Option<Self> {
        EXPORTING
            .compare_exchange(
                NOT_EXPORTING,
                cluster_id,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
            .then_some(Self)
    }
}

impl Drop for ExportSlot {
    fn drop(&mut self) {
        EXPORTING.store(NOT_EXPORTING, Ordering::Release);
    }
}

#[must_use]
pub fn exporting(cluster_id: ClusterId) -> bool {
    EXPORTING.load(Ordering::Acquire) == cluster_id
}

impl Actions {
    fn claim_export(&self, cluster_id: ClusterId) -> Option<ExportSlot> {
        if super::modpacks::modpack_job_running(cluster_id) {
            self.notify("Wait for the install to finish")
                .body("This instance is still being installed or updated. Export it once that's done.")
                .error()
                .send();
            return None;
        }
        if oneclient_core::clusters::is_resetting(cluster_id) {
            self.notify("This instance is being reset")
                .body("Wait for the reset to finish, then try again.")
                .error()
                .send();
            return None;
        }
        if super::external_import::import_running() {
            self.notify("Wait for the import to finish")
                .body("Instances can't be exported while an import is running.")
                .error()
                .send();
            return None;
        }
        let slot = ExportSlot::claim(cluster_id);
        if slot.is_none() {
            self.notify("An export is already running")
                .body("Wait for it to finish, then try again.")
                .error()
                .send();
        }
        slot
    }

    pub fn export_cluster(&self, cluster_id: ClusterId, options: ExportOptions) {
        let Some(slot) = self.claim_export(cluster_id) else {
            return;
        };
        let actions = self.clone();
        spawn_forever(async move {
            let _slot = slot;
            let Ok(state) = launcher::state() else { return };

            let cluster = match state.clusters.get(cluster_id).await {
                Ok(cluster) => cluster,
                Err(err) => {
                    actions.report_export_failure(&err.to_string());
                    return;
                }
            };

            let Some(dest) = pick_export_file(&cluster.name).await else {
                return;
            };

            let exported = off_ui({
                let state = state.clone();
                let dest = dest.clone();
                let title = format!("Exporting {}", cluster.name);
                async move {
                    let session = GroupedProgressSession::start(&state.services.events, title);
                    let exported =
                        export_cluster_with(&state, cluster_id, &dest, options, Some(&session))
                            .await;
                    if exported.is_ok() {
                        session.finish();
                    } else {
                        session.abort();
                    }
                    exported
                }
            })
            .await;

            match exported {
                Ok(report) => actions.report_export(&cluster.name, &dest, &report),
                Err(err) => actions.report_export_failure(&err.to_string()),
            }
        });
    }

    pub fn share_cluster(&self, cluster_id: ClusterId, options: ExportOptions) {
        self.start_share(cluster_id, ShareJob::Everything(options));
    }

    pub fn share_settings_only(&self, cluster_id: ClusterId, settings: SettingsChoice) {
        self.start_share(cluster_id, ShareJob::SettingsOnly(settings));
    }

    pub fn share_as_collaborator(&self, cluster_id: ClusterId, push: LinkedPush) {
        self.start_share(cluster_id, ShareJob::Linked(push));
    }

    fn start_share(&self, cluster_id: ClusterId, job: ShareJob) {
        self.set_share(cluster_id, ShareProgress::Working);
        let Some(slot) = self.claim_export(cluster_id) else {
            self.set_share(
                cluster_id,
                ShareProgress::Failed(
                    "This instance is busy right now. Try again once it's done.".into(),
                ),
            );
            return;
        };
        let actions = self.clone();
        spawn_forever(async move {
            let _slot = slot;
            let Ok(state) = launcher::state() else { return };

            let cluster = match state.clusters.get(cluster_id).await {
                Ok(cluster) => cluster,
                Err(err) => {
                    actions.report_share_failure(cluster_id, &err.to_string());
                    return;
                }
            };

            let shared = off_ui({
                let state = state.clone();
                let title = format!("Sharing {}", cluster.name);
                async move {
                    let session = GroupedProgressSession::start(&state.services.events, title);
                    let shared = async {
                        let store = sharing::default_store(state.auth.clone());
                        let ledger = sharing::default_ledger()?;
                        match job {
                            ShareJob::Everything(options) => {
                                sharing::share_cluster(
                                    &state,
                                    cluster_id,
                                    options,
                                    store.as_ref(),
                                    &ledger,
                                    Some(&session),
                                )
                                .await
                            }
                            ShareJob::Linked(push) => {
                                let links = sharing::default_linked_ledger()?;
                                sharing::push_linked(
                                    &state,
                                    cluster_id,
                                    push,
                                    store.as_ref(),
                                    &links,
                                    Some(&session),
                                )
                                .await
                            }
                            ShareJob::SettingsOnly(settings) => {
                                sharing::share_settings_only(
                                    &state,
                                    cluster_id,
                                    settings,
                                    store.as_ref(),
                                    &ledger,
                                    Some(&session),
                                )
                                .await
                            }
                        }
                    }
                    .await;
                    if shared.is_ok() {
                        session.finish();
                    } else {
                        session.abort();
                    }
                    shared
                }
            })
            .await;

            match shared {
                Ok(shared) => actions.report_share(cluster_id, &cluster.name, &shared),
                Err(err) => actions.report_share_failure(cluster_id, &err.to_string()),
            }
        });
    }

    pub fn stop_sharing(&self, cluster_id: ClusterId, ready: ShareReady) {
        if matches!(
            self.share_progress(cluster_id),
            Some(ShareProgress::Stopping(_) | ShareProgress::Working)
        ) {
            return;
        }
        self.set_share(cluster_id, ShareProgress::Stopping(ready.clone()));
        let actions = self.clone();
        spawn_forever(async move {
            let stopped = off_ui(async move {
                let state = launcher::state()?;
                let store = sharing::default_store(state.auth.clone());
                let ledger = sharing::default_ledger()?;
                sharing::stop_sharing(cluster_id, store.as_ref(), &ledger).await
            })
            .await;
            let progress = match stopped {
                Ok(_) => ShareProgress::Stopped,
                Err(err) => {
                    tracing::warn!(error = %err, "could not stop sharing an instance");
                    actions
                        .notify("Couldn't stop sharing")
                        .body(err.to_string())
                        .error()
                        .send();
                    ShareProgress::Ready(ready)
                }
            };
            actions.update_share(cluster_id, progress);
        });
    }

    pub fn dismiss_share(&self, cluster_id: ClusterId) {
        let mut guard = self.station.clone().write_channel(AppChannel::Share);
        guard.shares.remove(&cluster_id);
    }

    fn share_progress(&self, cluster_id: ClusterId) -> Option<ShareProgress> {
        self.station.peek().shares.get(&cluster_id).cloned()
    }

    fn set_share(&self, cluster_id: ClusterId, progress: ShareProgress) {
        let mut guard = self.station.clone().write_channel(AppChannel::Share);
        guard.shares.insert(cluster_id, progress);
    }

    fn update_share(&self, cluster_id: ClusterId, progress: ShareProgress) -> bool {
        let mut guard = self.station.clone().write_channel(AppChannel::Share);
        match guard.shares.get_mut(&cluster_id) {
            Some(current) => {
                *current = progress;
                true
            }
            None => false,
        }
    }

    pub fn import_share(&self, input: String) {
        if ShareCode::parse(&input).is_none() {
            self.notify("That isn't a share code")
                .body("Share codes look like ABCD-EFGH. Check it and try again.")
                .error()
                .send();
            return;
        }
        let actions = self.clone();
        spawn_forever(async move {
            let fetched = off_ui(async move {
                let state = launcher::state()?;
                let store = sharing::default_store(state.auth.clone());
                sharing::fetch_share(store.as_ref(), &input).await
            })
            .await;
            match fetched {
                Ok(fetched) => {
                    pending_links(|links| {
                        links.insert(fetched.path.clone(), (fetched.code, fetched.remote))
                    });
                    actions.install_modpack(ModpackSource::File(fetched.path));
                }
                Err(err) => {
                    tracing::warn!(error = %err, "could not fetch a shared instance");
                    actions
                        .notify("Couldn't open the share code")
                        .body(err.to_string())
                        .error()
                        .send();
                }
            }
        });
    }

    pub(super) async fn link_installed(&self, source: &ModpackSource, cluster_id: ClusterId) {
        let ModpackSource::File(path) = source else {
            return;
        };
        let Some((code, remote)) = pending_links(|links| links.remove(path)) else {
            return;
        };
        let Ok(state) = launcher::state() else { return };
        let path = path.clone();
        let linked = off_ui(async move {
            let ledger = sharing::default_linked_ledger()?;
            sharing::link_import(&state, &ledger, cluster_id, &code, &remote, &path).await
        })
        .await;
        if let Err(err) = linked {
            tracing::warn!(cluster_id, error = %err, "could not link the imported instance to its share code");
        }
    }

    pub(crate) async fn share_update_before_launch(
        &self,
        state: &Arc<LauncherState>,
        cluster_id: ClusterId,
    ) {
        let check = off_ui({
            let state = state.clone();
            async move {
                let store = sharing::default_store(state.auth.clone());
                let ledger = sharing::default_linked_ledger()?;
                sharing::check_update(store.as_ref(), &ledger, cluster_id).await
            }
        });
        let update = match tokio::time::timeout(UPDATE_CHECK_LIMIT, check).await {
            Ok(Ok(Some(update))) => update,
            Ok(Ok(None)) => return,
            Ok(Err(err)) => {
                tracing::info!(cluster_id, error = %err, "could not check the share code for updates; launching as is");
                return;
            }
            Err(_) => {
                tracing::info!(
                    cluster_id,
                    "the share update check took too long; launching as is"
                );
                return;
            }
        };
        let name = state
            .clusters
            .get(cluster_id)
            .await
            .map(|cluster| cluster.name)
            .unwrap_or_else(|_| "This instance".to_string());

        match update {
            ShareUpdate::Current => {}
            ShareUpdate::Ended { link } => {
                let unlinked = off_ui(async move {
                    let ledger = sharing::default_linked_ledger()?;
                    sharing::unlink(&ledger, link.cluster_id).await
                })
                .await;
                if let Err(err) = unlinked {
                    tracing::warn!(cluster_id, error = %err, "could not unlink an ended share");
                }
                self.notify(format!("{name} no longer gets updates"))
                    .body("The person who shared it stopped sharing, or the code expired. Your copy stays as it is.")
                    .send();
            }
            ShareUpdate::Available { link, changes, .. } => {
                let mut body = format!(
                    "The person who shared {name} updated it under {}.",
                    link.code.pretty()
                );
                for line in changes.describe() {
                    body.push(' ');
                    body.push_str(&line);
                }
                body.push_str(" Mods you added yourself stay.");
                let prompt = Prompt::new(format!("Update {name}?"), body)
                    .option(
                        Choice::primary("share.update", "Update and play"),
                        UpdateAnswer::Update,
                    )
                    .option(
                        Choice::new("share.skip", "Play without updating"),
                        UpdateAnswer::Skip,
                    )
                    .option(
                        Choice::new("share.unlink", "Stop updates"),
                        UpdateAnswer::Unlink,
                    )
                    .dismiss("Not now");
                let answer = match self.events().ask(prompt).await {
                    Ok(Some(chosen)) => chosen.value,
                    Ok(None) => UpdateAnswer::Skip,
                    Err(err) => {
                        tracing::warn!(cluster_id, "share update prompt failed: {err:#}");
                        UpdateAnswer::Skip
                    }
                };
                match answer {
                    UpdateAnswer::Skip => {}
                    UpdateAnswer::Unlink => {
                        let unlinked = off_ui(async move {
                            let ledger = sharing::default_linked_ledger()?;
                            sharing::unlink(&ledger, cluster_id).await
                        })
                        .await;
                        if let Err(err) = unlinked {
                            tracing::warn!(cluster_id, error = %err, "could not stop share updates");
                        }
                    }
                    UpdateAnswer::Update => self.apply_share_update(state, cluster_id, &name).await,
                }
            }
        }
    }

    async fn apply_share_update(
        &self,
        state: &Arc<LauncherState>,
        cluster_id: ClusterId,
        name: &str,
    ) {
        let applied = off_ui({
            let state = state.clone();
            let title = format!("Updating {name}");
            async move {
                let session = GroupedProgressSession::start(&state.services.events, title);
                let applied = async {
                    let store = sharing::default_store(state.auth.clone());
                    let ledger = sharing::default_linked_ledger()?;
                    sharing::apply_update(&state, store.as_ref(), &ledger, cluster_id).await
                }
                .await;
                if applied.is_ok() {
                    session.finish();
                } else {
                    session.abort();
                }
                applied
            }
        })
        .await;
        if let Ok(state) = launcher::state() {
            state
                .services
                .events
                .signal(oneclient_events::Signal::ClustersChanged);
        }
        crate::hooks::invalidate_cluster_queries().await;
        match applied {
            Ok(applied) if applied != cluster_id => {
                self.notify(format!("{name} moved to a new Minecraft version"))
                    .body("The update was installed as a new instance so your current one stays playable. Play the new one from your instances.")
                    .send();
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "could not update from the share code");
                self.notify("Couldn't update the instance")
                    .body(format!("{err} Playing your current copy."))
                    .error()
                    .send();
            }
        }
    }

    fn report_share(&self, cluster_id: ClusterId, cluster_name: &str, shared: &SharedInstance) {
        let code = shared.published.code.pretty();
        let ready = ShareReady {
            details: export_details(&shared.report).trim().to_string(),
            incomplete: !shared.report.missing.is_empty(),
            updated: shared.updated,
            ..ready_from(&shared.published)
        };
        if self.update_share(cluster_id, ShareProgress::Ready(ready)) {
            return;
        }

        let copied = freya::text_edit::Clipboard::set(code.clone())
            .inspect_err(|err| tracing::warn!("clipboard copy failed: {err:?}"))
            .is_ok();
        let mut body = if shared.updated {
            format!("{cluster_name} is updated under {code}.")
        } else {
            format!("{cluster_name} is shared as {code}.")
        };
        if copied {
            body.push_str(" The code is copied.");
        }
        body.push_str(&export_details(&shared.report));

        let notification = self
            .notify("Share code ready")
            .body(body)
            .icon(IconType::Users01);
        if shared.report.missing.is_empty() {
            notification.send();
        } else {
            notification.error().send();
        }
    }

    fn report_share_failure(&self, cluster_id: ClusterId, reason: &str) {
        tracing::error!("could not share an instance: {reason}");
        if self.update_share(cluster_id, ShareProgress::Failed(reason.to_string())) {
            return;
        }
        self.notify("Couldn't share the instance")
            .body(reason.to_string())
            .error()
            .send();
    }

    fn report_export(
        &self,
        cluster_name: &str,
        dest: &std::path::Path,
        report: &ShareExportReport,
    ) {
        let file = dest
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| cluster_name.to_string());

        let mut body = format!("Saved {cluster_name} as {file}.");
        body.push_str(&export_details(report));

        let notification = self
            .notify("Instance exported")
            .body(body)
            .icon(IconType::Users01);
        if report.missing.is_empty() {
            notification.send();
        } else {
            notification.error().send();
        }
    }

    fn report_export_failure(&self, reason: &str) {
        tracing::error!("could not export an instance: {reason}");
        self.notify("Couldn't export the instance")
            .body(reason.to_string())
            .error()
            .send();
    }
}

enum ShareJob {
    Everything(ExportOptions),
    SettingsOnly(SettingsChoice),
    Linked(LinkedPush),
}

fn ready_from(published: &PublishedShare) -> ShareReady {
    ShareReady {
        code: published.code.pretty(),
        link: published.link.clone(),
        updated: false,
        expires: published.expires_at.map(|at| {
            at.with_timezone(&chrono::Local)
                .format("%-d %B %Y")
                .to_string()
        }),
        details: String::new(),
        incomplete: false,
    }
}

pub async fn load_owner_check(cluster_id: ClusterId) -> OwnerCheck {
    let Ok(state) = launcher::state() else {
        return OwnerCheck::NotShared;
    };
    let collaborator_state = state.clone();
    let ledger_entry = off_ui(async move {
        let store = sharing::default_store(state.auth.clone());
        let ledger = sharing::default_ledger()?;
        let entry = ledger.find(cluster_id, store.name()).await?;
        let Some(entry) = entry else {
            return oneclient_core::LauncherResult::Ok(None);
        };
        let status = sharing::owner_status(&state, cluster_id, store.as_ref(), &ledger).await;
        Ok(Some((entry.share, status)))
    })
    .await;
    match ledger_entry {
        Ok(None) => collaborator_check(collaborator_state, cluster_id).await,
        Ok(Some((_, Ok(Some(status))))) => OwnerCheck::Shared(OwnerShare {
            ready: ready_from(&status.share),
            options: status.options,
            content_changed: status.content_changed,
            settings_changed: status.settings_changed,
            behind: status.behind,
        }),
        Ok(Some((share, Ok(None)))) => OwnerCheck::Unknown(ready_from(&share)),
        Ok(Some((share, Err(err)))) => {
            tracing::info!(cluster_id, error = %err, "could not check the shared instance for changes");
            OwnerCheck::Unknown(ready_from(&share))
        }
        Err(err) => {
            tracing::warn!(cluster_id, error = %err, "could not read the share list");
            OwnerCheck::NotShared
        }
    }
}

pub async fn load_is_shared(cluster_id: ClusterId) -> bool {
    let Ok(state) = launcher::state() else {
        return false;
    };
    off_ui(async move {
        let store = sharing::default_store(state.auth.clone());
        let ledger = sharing::default_ledger()?;
        oneclient_core::LauncherResult::Ok(ledger.find(cluster_id, store.name()).await?.is_some())
    })
    .await
    .unwrap_or(false)
}

async fn collaborator_check(state: Arc<LauncherState>, cluster_id: ClusterId) -> OwnerCheck {
    let checked = off_ui(async move {
        let links = sharing::default_linked_ledger()?;
        let Some(link) = links.find(cluster_id).await? else {
            return oneclient_core::LauncherResult::Ok(None);
        };
        let store = sharing::default_store(state.auth.clone());
        let remote = store.describe(&link.code).await?;
        Ok(
            (remote.role == ShareRole::Collaborator).then(|| ShareReady {
                expires: remote.expires_at.map(|at| {
                    at.with_timezone(&chrono::Local)
                        .format("%-d %B %Y")
                        .to_string()
                }),
                ..ready_from(&PublishedShare {
                    link: link.code.web_link(),
                    code: link.code,
                    manage_token: String::new(),
                    expires_at: None,
                })
            }),
        )
    })
    .await;
    match checked {
        Ok(Some(ready)) => OwnerCheck::Collaborator(ready),
        Ok(None) => OwnerCheck::NotShared,
        Err(err) => {
            tracing::info!(cluster_id, error = %err, "could not check whether this copy can update its share");
            OwnerCheck::NotShared
        }
    }
}

fn share_code(pretty: &str) -> Result<ShareCode, String> {
    ShareCode::parse(pretty).ok_or_else(|| "That share code isn't valid.".to_string())
}

pub async fn load_collaborators(code: String) -> Result<Vec<Collaborator>, String> {
    let code = share_code(&code)?;
    let state = launcher::state().map_err(|err| err.to_string())?;
    off_ui(async move {
        let store = sharing::default_store(state.auth.clone());
        store.collaborators(&code).await
    })
    .await
    .map_err(|err| err.to_string())
}

pub async fn add_collaborator(code: String, username: String) -> Result<Collaborator, String> {
    let code = share_code(&code)?;
    let state = launcher::state().map_err(|err| err.to_string())?;
    off_ui(async move {
        let store = sharing::default_store(state.auth.clone());
        store.add_collaborator(&code, username.trim()).await
    })
    .await
    .map_err(|err| err.to_string())
}

pub async fn remove_collaborator(code: String, uuid: uuid::Uuid) -> Result<(), String> {
    let code = share_code(&code)?;
    let state = launcher::state().map_err(|err| err.to_string())?;
    off_ui(async move {
        let store = sharing::default_store(state.auth.clone());
        store.remove_collaborator(&code, uuid).await
    })
    .await
    .map_err(|err| err.to_string())
}

pub async fn load_setting_files(cluster_id: ClusterId) -> Vec<String> {
    let Ok(state) = launcher::state() else {
        return Vec::new();
    };
    match off_ui(async move { list_setting_files(&state, cluster_id).await }).await {
        Ok(files) => files.into_iter().map(|file| file.path).collect(),
        Err(err) => {
            tracing::warn!(cluster_id, error = %err, "could not list the instance's settings files");
            Vec::new()
        }
    }
}

pub(super) fn share_preview(source: &ModpackSource) -> Option<SharedPreview> {
    let ModpackSource::File(path) = source else {
        return None;
    };
    pending_links(|links| {
        links
            .get(path)
            .map(|(code, remote)| preview_of(code, remote))
    })
}

fn preview_of(code: &ShareCode, remote: &RemoteShare) -> SharedPreview {
    let Some(contents) = &remote.contents else {
        return SharedPreview {
            code: code.pretty(),
            files: Vec::new(),
            settings: 0,
            truncated: false,
        };
    };
    let mut files: Vec<SharedPreviewFile> = contents
        .files
        .iter()
        .map(|file| SharedPreviewFile {
            name: file.name().to_string(),
            source: source_label(file.source.as_deref()),
            verified: file.is_verified(),
        })
        .collect();
    files.sort_by(|a, b| {
        a.verified
            .cmp(&b.verified)
            .then_with(|| a.name.cmp(&b.name))
    });
    let truncated = contents.truncated || files.len() > MAX_PREVIEW_FILES;
    files.truncate(MAX_PREVIEW_FILES);
    SharedPreview {
        code: code.pretty(),
        files,
        settings: contents.settings.len(),
        truncated,
    }
}

fn source_label(source: Option<&str>) -> &'static str {
    match source {
        Some("modrinth") => "Modrinth",
        Some("curseforge") => "CurseForge",
        Some("github") => "GitHub",
        Some("gitlab") => "GitLab",
        Some("embedded") => "Inside the share",
        _ => "Other website",
    }
}

async fn pick_export_file(cluster_name: &str) -> Option<PathBuf> {
    let stem = ClusterManager::sanitize_name(cluster_name);
    let stem = if stem.is_empty() {
        "instance".to_string()
    } else {
        stem
    };

    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Export instance")
        .set_file_name(format!("{stem}.{SHARE_EXTENSION}"))
        .add_filter("OneClient instance", &[SHARE_EXTENSION]);
    if let Some(dir) =
        directories::UserDirs::new().and_then(|dirs| dirs.download_dir().map(PathBuf::from))
    {
        dialog = dialog.set_directory(dir);
    }

    let mut path = dialog.save_file().await?.path().to_path_buf();
    if path.extension().and_then(|ext| ext.to_str()) != Some(SHARE_EXTENSION) {
        path.set_extension(SHARE_EXTENSION);
    }
    Some(path)
}

fn export_details(report: &ShareExportReport) -> String {
    let mut body = String::new();
    if !report.embedded.is_empty() {
        body.push_str(&format!(
            " {} not on Modrinth or CurseForge went inside the file.",
            count_label(report.embedded.len(), "file", "files")
        ));
    }
    if report.settings > 0 {
        body.push_str(&format!(
            " Included {}.",
            count_label(report.settings, "settings file", "settings files")
        ));
    }
    if !report.skipped_settings.is_empty() {
        body.push_str(&format!(
            " Skipped {} that were too large: {}.",
            count_label(
                report.skipped_settings.len(),
                "settings file",
                "settings files"
            ),
            listed(&report.skipped_settings)
        ));
    }
    if !report.missing.is_empty() {
        body.push_str(&format!(
            " Left out {} that could not be found: {}.",
            count_label(report.missing.len(), "file", "files"),
            listed(&report.missing)
        ));
    }
    body
}

fn count_label(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn listed(names: &[String]) -> String {
    let shown = names
        .iter()
        .take(MAX_LISTED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().saturating_sub(MAX_LISTED) {
        0 => shown,
        extra => format!("{shown} and {extra} more"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_export_runs_at_a_time() {
        let first = ExportSlot::claim(7).expect("nothing is exporting yet");
        assert!(exporting(7));
        assert!(!exporting(8));
        assert!(ExportSlot::claim(8).is_none());
        drop(first);
        assert!(!exporting(7));
        assert!(ExportSlot::claim(8).is_some());
    }

    #[test]
    fn counts_read_naturally() {
        assert_eq!(count_label(1, "file", "files"), "1 file");
        assert_eq!(count_label(4, "file", "files"), "4 files");
    }

    #[test]
    fn long_lists_are_cut_short() {
        let names: Vec<String> = (1..=5).map(|i| format!("m{i}")).collect();
        assert_eq!(listed(&names[..2]), "m1, m2");
        assert_eq!(listed(&names), "m1, m2, m3 and 2 more");
    }
}
