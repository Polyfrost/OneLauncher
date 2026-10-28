mod code;
pub(crate) mod http;
mod ledger;
mod linked;
mod local;
mod store;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use oneclient_auth::AuthService;
use oneclient_content::modpacks::{self, SHARE_EXTENSION, SHARE_FORMAT, ShareExportReport};
use oneclient_events::GroupedProgressSession;

pub use code::{APP_LINK_HOST, CODE_LENGTH, SHARE_LINK_BASE, SHARE_LINK_HOSTS, ShareCode};
pub use http::HttpShareStore;
pub use ledger::{LedgerEntry, PushState, ShareLedger};
pub use linked::{
    LinkedLedger, LinkedShare, ShareChanges, ShareUpdate, apply_update, check_update,
    default_linked_ledger, link_import, unlink,
};
pub use local::LocalShareStore;
pub use store::{
    Collaborator, ContentEntry, MAX_NAME_CHARS, MAX_SHARE_BYTES, PublishedShare, RemoteShare,
    ShareContents, ShareMeta, ShareRole, ShareStore, SharingError,
};

use crate::LauncherResult;
use crate::clusters::{
    ExportMode, ExportOptions, SettingsChoice, collect_chosen_settings, export_cluster_with,
};
use crate::state::LauncherState;

pub const SHARE_DIR_ENV: &str = "ONECLIENT_SHARE_DIR";

const PUBLISHED_FILE: &str = "published.json";
const LINKED_FILE: &str = "linked.json";

const INDEX_ENTRY: &str = "modrinth.index.json";

#[derive(Debug, Clone)]
pub struct SharedInstance {
    pub published: PublishedShare,
    pub meta: ShareMeta,
    pub report: ShareExportReport,
    pub updated: bool,
}

#[derive(Debug, Clone)]
pub struct FetchedShare {
    pub code: ShareCode,
    pub meta: ShareMeta,
    pub remote: RemoteShare,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum LinkedPush {
    Everything(ExportOptions),
    SettingsOnly(SettingsChoice),
}

#[derive(Debug, Clone)]
pub struct OwnerStatus {
    pub share: PublishedShare,
    pub options: ExportOptions,
    pub content_changed: bool,
    pub settings_changed: bool,
    pub behind: bool,
}

pub fn sharing_dir() -> LauncherResult<PathBuf> {
    Ok(oneclient_common::paths::data_dir()?.join("shares"))
}

pub fn incoming_dir() -> LauncherResult<PathBuf> {
    Ok(sharing_dir()?.join("incoming"))
}

pub fn default_store(auth: Arc<AuthService>) -> Arc<dyn ShareStore> {
    match std::env::var_os(SHARE_DIR_ENV).filter(|dir| !dir.is_empty()) {
        Some(dir) => Arc::new(LocalShareStore::new(PathBuf::from(dir))),
        None => Arc::new(HttpShareStore::new(auth)),
    }
}

pub fn default_ledger() -> LauncherResult<ShareLedger> {
    Ok(ShareLedger::at(sharing_dir()?.join(PUBLISHED_FILE)))
}

fn published_copy(cluster_id: i64) -> LauncherResult<PathBuf> {
    Ok(sharing_dir()?
        .join("published")
        .join(format!("{cluster_id}.{SHARE_EXTENSION}")))
}

async fn keep_published_copy(cluster_id: i64, payload: &Path) {
    let kept = async {
        let dest = published_copy(cluster_id)?;
        if let Some(parent) = dest.parent() {
            polyio::create_dir_all(parent).await?;
        }
        let staging = polyio::temp_sibling(&dest);
        polyio::copy(payload, &staging).await?;
        polyio::rename(&staging, &dest).await?;
        LauncherResult::Ok(())
    }
    .await;
    if let Err(err) = kept {
        tracing::warn!(cluster_id, error = %err, "could not keep a copy of the shared instance");
    }
}

pub(crate) async fn push_state(
    payload: &Path,
    options: ExportOptions,
) -> LauncherResult<PushState> {
    let fingerprints = modpacks::share_fingerprints(payload).await?;
    Ok(PushState {
        options,
        content: fingerprints.content,
        settings: fingerprints.settings,
    })
}

pub(crate) async fn refresh_push_state(
    ledger: &ShareLedger,
    store: &str,
    cluster_id: i64,
    payload: &Path,
) {
    let refreshed = async {
        let Some(entry) = ledger.find(cluster_id, store).await? else {
            return LauncherResult::Ok(());
        };
        let options = entry.push.map_or_else(
            || ExportOptions::new(ExportMode::OneClient),
            |push| push.options,
        );
        let push = push_state(payload, options).await?;
        ledger
            .remember(cluster_id, store, &entry.share, Some(push))
            .await?;
        keep_published_copy(cluster_id, payload).await;
        Ok(())
    }
    .await;
    if let Err(err) = refreshed {
        tracing::warn!(cluster_id, error = %err, "could not record the update as the shared version");
    }
}

async fn update_owned(
    store: &dyn ShareStore,
    links: &LinkedLedger,
    cluster_id: i64,
    share: &PublishedShare,
    payload: &Path,
    meta: &ShareMeta,
) -> Result<PublishedShare, SharingError> {
    match links.find(cluster_id).await? {
        Some(link) if link.code == share.code => {
            store.update_from(share, link.revision, payload, meta).await
        }
        _ => store.update(share, payload, meta).await,
    }
}

async fn follow_own_share(
    state: &Arc<LauncherState>,
    links: &LinkedLedger,
    cluster_id: i64,
    store: &dyn ShareStore,
    code: &ShareCode,
    payload: &Path,
) {
    let followed = async {
        let remote = store.describe(code).await?;
        link_import(state, links, cluster_id, code, &remote, payload).await?;
        LauncherResult::Ok(())
    }
    .await;
    if let Err(err) = followed {
        tracing::warn!(cluster_id, code = %code, error = %err, "could not follow the instance's own share");
    }
}

async fn upload(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    payload: &Path,
    meta: &ShareMeta,
    options: ExportOptions,
    store: &dyn ShareStore,
    ledger: &ShareLedger,
) -> LauncherResult<(PublishedShare, bool)> {
    let push = push_state(payload, options).await?;
    let links = ledger.linked();

    if let Some(previous) = ledger.find(cluster_id, store.name()).await? {
        match update_owned(store, &links, cluster_id, &previous.share, payload, meta).await {
            Ok(published) => {
                ledger
                    .remember(cluster_id, store.name(), &published, Some(push))
                    .await?;
                keep_published_copy(cluster_id, payload).await;
                follow_own_share(state, &links, cluster_id, store, &published.code, payload).await;
                return Ok((published, true));
            }
            Err(SharingError::NotFound(_) | SharingError::Gone(_) | SharingError::Forbidden(_)) => {
                tracing::info!(cluster_id, code = %previous.share.code, "the earlier share is gone; publishing a new one");
            }
            Err(err) => return Err(err.into()),
        }
    }

    let published = store.publish(payload, meta).await?;
    ledger
        .remember(cluster_id, store.name(), &published, Some(push))
        .await?;
    keep_published_copy(cluster_id, payload).await;
    follow_own_share(state, &links, cluster_id, store, &published.code, payload).await;
    Ok((published, false))
}

#[tracing::instrument(level = "debug")]
pub async fn describe_payload(path: &Path) -> LauncherResult<ShareMeta> {
    let size = polyio::stat(path).await?.len();
    if size > MAX_SHARE_BYTES {
        return Err(SharingError::TooLarge {
            size,
            limit: MAX_SHARE_BYTES,
        }
        .into());
    }
    let share = modpacks::read_cluster_share(path).await?.ok_or_else(|| {
        SharingError::Corrupt("the file is not a OneClient instance export".to_string())
    })?;
    let sha512 = polyio::sha512_file(path).await?;
    Ok(ShareMeta {
        format: share.format,
        name: share.name.chars().take(MAX_NAME_CHARS).collect(),
        mc_version: share.mc_version,
        mc_loader: share.mc_loader,
        kind: share.kind,
        size,
        sha512,
        created_at: None,
    })
}

#[tracing::instrument(skip(state, store, ledger, progress))]
pub async fn share_cluster(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    options: ExportOptions,
    store: &dyn ShareStore,
    ledger: &ShareLedger,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<SharedInstance> {
    let staging = polyio::tempdir().await?;
    let payload = staging.dir_path().join(format!("share.{SHARE_EXTENSION}"));
    let report =
        export_cluster_with(state, cluster_id, &payload, options.clone(), progress).await?;
    let meta = describe_payload(&payload).await?;
    let (published, updated) =
        upload(state, cluster_id, &payload, &meta, options, store, ledger).await?;
    Ok(SharedInstance {
        published,
        meta,
        report,
        updated,
    })
}

#[tracing::instrument(skip(state, store, ledger, progress))]
pub async fn share_settings_only(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    settings: SettingsChoice,
    store: &dyn ShareStore,
    ledger: &ShareLedger,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<SharedInstance> {
    let entry = ledger
        .find(cluster_id, store.name())
        .await?
        .ok_or_else(|| SharingError::Unavailable("share the whole instance first".into()))?;
    let cluster = state.clusters.get(cluster_id).await?;
    let child = progress.map(|session| {
        let child = session.child(
            cluster.name.clone(),
            1,
            oneclient_events::TaskCategory::Packages,
        );
        child.set_phase(oneclient_events::TaskPhase::Exporting);
        child
    });

    let staging = polyio::tempdir().await?;
    let mut base = published_copy(cluster_id)?;
    if !polyio::try_exists(&base).await? || modpacks::read_cluster_share(&base).await.is_err() {
        base = staging.dir_path().join(format!("base.{SHARE_EXTENSION}"));
        store.fetch(&entry.share.code, &base).await?;
    }

    let scan = collect_chosen_settings(&cluster.game_dir()?, &settings).await?;
    let payload = staging.dir_path().join(format!("share.{SHARE_EXTENSION}"));
    let report = modpacks::replace_share_settings(&base, &payload, &scan).await?;
    let meta = describe_payload(&payload).await?;

    let mut options = entry.push.as_ref().map_or_else(
        || ExportOptions::new(crate::clusters::ExportMode::OneClient),
        |push| push.options.clone(),
    );
    options.settings = settings;
    let push = push_state(&payload, options).await?;
    let links = ledger.linked();
    let published = update_owned(store, &links, cluster_id, &entry.share, &payload, &meta).await?;
    ledger
        .remember(cluster_id, store.name(), &published, Some(push))
        .await?;
    keep_published_copy(cluster_id, &payload).await;
    follow_own_share(state, &links, cluster_id, store, &published.code, &payload).await;
    if let Some(child) = child {
        child.finish();
    }
    Ok(SharedInstance {
        published,
        meta,
        report,
        updated: true,
    })
}

#[tracing::instrument(skip(state, store, links, progress))]
pub async fn push_linked(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    push: LinkedPush,
    store: &dyn ShareStore,
    links: &LinkedLedger,
    progress: Option<&GroupedProgressSession>,
) -> LauncherResult<SharedInstance> {
    let link = links.find(cluster_id).await?.ok_or_else(|| {
        SharingError::Unavailable("this instance isn't linked to a share code".into())
    })?;
    let staging = polyio::tempdir().await?;
    let payload = staging.dir_path().join(format!("share.{SHARE_EXTENSION}"));
    let report = match push {
        LinkedPush::Everything(options) => {
            export_cluster_with(state, cluster_id, &payload, options, progress).await?
        }
        LinkedPush::SettingsOnly(settings) => {
            let base = staging.dir_path().join(format!("base.{SHARE_EXTENSION}"));
            let remote = store.fetch(&link.code, &base).await?;
            if remote.revision != link.revision {
                return Err(SharingError::Outdated(link.code.clone()).into());
            }
            let cluster = state.clusters.get(cluster_id).await?;
            let scan = collect_chosen_settings(&cluster.game_dir()?, &settings).await?;
            modpacks::replace_share_settings(&base, &payload, &scan).await?
        }
    };
    let meta = describe_payload(&payload).await?;
    let share = PublishedShare {
        link: link.code.web_link(),
        code: link.code.clone(),
        manage_token: String::new(),
        expires_at: None,
    };
    let published = store
        .update_from(&share, link.revision, &payload, &meta)
        .await?;
    let remote = store.describe(&link.code).await?;
    link_import(state, links, cluster_id, &link.code, &remote, &payload).await?;
    tracing::info!(cluster_id, code = %link.code, revision = remote.revision, "pushed an update to a share as a collaborator");
    Ok(SharedInstance {
        published,
        meta,
        report,
        updated: true,
    })
}

#[tracing::instrument(skip(state, store, ledger))]
pub async fn owner_status(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    store: &dyn ShareStore,
    ledger: &ShareLedger,
) -> LauncherResult<Option<OwnerStatus>> {
    let Some(entry) = ledger.find(cluster_id, store.name()).await? else {
        return Ok(None);
    };
    let Some(push) = entry.push else {
        return Ok(Some(OwnerStatus {
            share: entry.share,
            options: ExportOptions::new(crate::clusters::ExportMode::OneClient),
            content_changed: true,
            settings_changed: true,
            behind: false,
        }));
    };
    let behind = match ledger.linked().find(cluster_id).await? {
        Some(link) => store
            .describe(&entry.share.code)
            .await
            .is_ok_and(|remote| remote.revision > link.revision),
        None => false,
    };

    let staging = polyio::tempdir().await?;
    let payload = staging.dir_path().join(format!("check.{SHARE_EXTENSION}"));
    export_cluster_with(state, cluster_id, &payload, push.options.clone(), None).await?;
    let current = modpacks::share_fingerprints(&payload).await?;
    Ok(Some(OwnerStatus {
        share: entry.share,
        content_changed: current.content != push.content,
        settings_changed: current.settings != push.settings,
        behind,
        options: push.options,
    }))
}

#[tracing::instrument(skip(store, ledger))]
pub async fn stop_sharing(
    cluster_id: i64,
    store: &dyn ShareStore,
    ledger: &ShareLedger,
) -> LauncherResult<Option<ShareCode>> {
    let Some(entry) = ledger.find(cluster_id, store.name()).await? else {
        return Ok(None);
    };
    match store.revoke(&entry.share).await {
        Ok(()) | Err(SharingError::NotFound(_) | SharingError::Gone(_)) => {}
        Err(err) => return Err(err.into()),
    }
    ledger.forget(cluster_id, store.name()).await?;
    if let Ok(copy) = published_copy(cluster_id) {
        polyio::remove_file(&copy).await.ok();
    }
    Ok(Some(entry.share.code))
}

#[tracing::instrument(skip(store))]
pub async fn fetch_share(store: &dyn ShareStore, input: &str) -> LauncherResult<FetchedShare> {
    fetch_share_into(store, input, &incoming_dir()?).await
}

#[tracing::instrument(skip(store))]
pub async fn fetch_share_into(
    store: &dyn ShareStore,
    input: &str,
    dir: &Path,
) -> LauncherResult<FetchedShare> {
    let code = ShareCode::parse(input).ok_or(SharingError::InvalidCode)?;
    let described = store.describe(&code).await?;
    check_meta(&described.meta)?;

    polyio::create_dir_all(dir).await?;
    let path = dir.join(format!("{}.{SHARE_EXTENSION}", code.as_str()));
    let staging = polyio::temp_sibling(&path);
    let fetched = async {
        let remote = store.fetch(&code, &staging).await?;
        check_meta(&remote.meta)?;
        verify_payload(&staging, &remote.meta).await?;
        polyio::rename(&staging, &path).await?;
        LauncherResult::Ok(remote)
    }
    .await;

    match fetched {
        Ok(remote) => {
            tracing::info!(code = %code, name = %remote.meta.name, revision = remote.revision, "fetched a shared instance");
            let remote = RemoteShare {
                contents: remote.contents.or(described.contents),
                ..remote
            };
            Ok(FetchedShare {
                code,
                meta: remote.meta.clone(),
                remote,
                path,
            })
        }
        Err(err) => {
            polyio::remove_file(&staging).await.ok();
            Err(err)
        }
    }
}

pub(crate) fn check_meta(meta: &ShareMeta) -> Result<(), SharingError> {
    if meta.size > MAX_SHARE_BYTES {
        return Err(SharingError::TooLarge {
            size: meta.size,
            limit: MAX_SHARE_BYTES,
        });
    }
    if meta.format > SHARE_FORMAT {
        return Err(SharingError::Corrupt(format!(
            "it was shared from a newer OneClient (format {}). Update OneClient to import it.",
            meta.format
        )));
    }
    Ok(())
}

pub(crate) async fn verify_payload(path: &Path, meta: &ShareMeta) -> LauncherResult<()> {
    let size = polyio::stat(path).await?.len();
    if size != meta.size {
        return Err(
            SharingError::Corrupt(format!("expected {} bytes but got {size}", meta.size)).into(),
        );
    }
    let sha512 = polyio::sha512_file(path).await?;
    if polyio::normalize_hash(&sha512) != polyio::normalize_hash(&meta.sha512) {
        return Err(
            SharingError::Corrupt("the download does not match its checksum".to_string()).into(),
        );
    }
    let names = polyio::zip_entry_names(path)
        .await
        .map_err(|err| SharingError::Corrupt(err.to_string()))?;
    if !names.iter().any(|name| name == INDEX_ENTRY) {
        return Err(SharingError::Corrupt("it is not a modpack file".to_string()).into());
    }
    modpacks::read_cluster_share(path).await?;
    Ok(())
}
