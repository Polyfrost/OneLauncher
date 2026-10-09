mod plan;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use oneclient_common::domain::ContentType;
use oneclient_content::packages::store::artifact_absolute_path;
use oneclient_content::packages::{LinkedArtifactInfo, PackageStore, read_jar_dependencies};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bisect as bisect_dao;
use oneclient_db::dao::cluster_bisect::BisectModState;
pub use oneclient_db::models::BisectExit;
use oneclient_events::Signal;
use thiserror::Error;

use crate::LauncherResult;
use crate::game::diagnosis::{MissingDependency, MissingTarget};
use crate::state::LauncherState;
use plan::ModSet;
pub use plan::launches_left;

#[derive(Debug, Error)]
pub enum BisectError {
    #[error("Close the game before changing which mods are being tested")]
    GameRunning,

    #[error("A problem mod search is already running for this cluster")]
    AlreadyRunning,

    #[error("No problem mod search is running for this cluster")]
    NotRunning,

    #[error("At least two enabled mods are needed to search for a problem mod")]
    TooFewMods,

    #[error("There is no earlier answer to undo")]
    NothingToUndo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectRole {
    Testing,
    Waiting,
    Helper,
    Cleared,
}

impl BisectRole {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Testing => "Testing",
            Self::Waiting => "Off this round",
            Self::Helper => "Needed by a tested mod",
            Self::Cleared => "Cleared",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectMod {
    pub hash: String,
    pub name: String,
    pub role: BisectRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectStatus {
    pub cluster_id: i64,
    pub round: i64,
    pub pending_exit: BisectExit,
    pub mods: Vec<BisectMod>,
    pub finished: bool,
    pub retry_note: Option<String>,
}

impl BisectStatus {
    #[must_use]
    pub fn suspects(&self) -> impl Iterator<Item = &BisectMod> {
        self.mods
            .iter()
            .filter(|m| matches!(m.role, BisectRole::Testing | BisectRole::Waiting))
    }

    #[must_use]
    pub fn suspect_count(&self) -> usize {
        self.suspects().count()
    }

    #[must_use]
    pub fn testing_count(&self) -> usize {
        self.mods
            .iter()
            .filter(|m| m.role == BisectRole::Testing)
            .count()
    }

    #[must_use]
    pub fn launches_left(&self) -> u32 {
        if self.finished {
            0
        } else {
            plan::launches_left(self.suspect_count())
        }
    }

    #[must_use]
    pub fn awaiting_answer(&self) -> bool {
        self.pending_exit != BisectExit::None && !self.finished
    }

    #[must_use]
    pub fn off_this_round(&self) -> impl Iterator<Item = &BisectMod> {
        self.mods
            .iter()
            .filter(|m| matches!(m.role, BisectRole::Waiting | BisectRole::Cleared))
    }

    #[must_use]
    pub fn role_of(&self, hash: &str) -> Option<BisectRole> {
        self.mods.iter().find(|m| m.hash == hash).map(|m| m.role)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectAnswer {
    Broken,
    Works,
}

static OPS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Session {
    round: i64,
    pending_exit: BisectExit,
    retry_note: Option<String>,
    hashes: Vec<String>,
    cleared: Vec<Option<i64>>,
    testing: ModSet,
    linked: HashMap<String, LinkedArtifactInfo>,
    edges: Vec<(String, String)>,
}

impl Session {
    fn suspects(&self) -> ModSet {
        (0..self.hashes.len())
            .filter(|&i| self.cleared[i].is_none() && self.linked.contains_key(&self.hashes[i]))
            .collect()
    }

    fn off(&self) -> ModSet {
        (0..self.hashes.len())
            .filter(|&i| !self.testing.contains(&i) && self.linked.contains_key(&self.hashes[i]))
            .collect()
    }

    fn index_of(&self, hash: &str) -> Option<usize> {
        self.hashes.iter().position(|h| h == hash)
    }

    fn name(&self, index: usize) -> String {
        self.linked.get(&self.hashes[index]).map_or_else(
            || self.hashes[index].clone(),
            |info| {
                info.display_name
                    .clone()
                    .unwrap_or_else(|| info.file_name.clone())
            },
        )
    }

    fn names(&self, indices: &ModSet) -> String {
        indices
            .iter()
            .map(|&i| self.name(i))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

struct Graph {
    requires: Vec<Vec<usize>>,
    provides: HashMap<String, usize>,
}

#[tracing::instrument(skip(state))]
pub async fn bisect_status(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<Option<BisectStatus>> {
    let Some(session) = load(state, cluster_id).await? else {
        return Ok(None);
    };
    Ok(Some(describe(cluster_id, &session)))
}

#[tracing::instrument(skip(state))]
pub async fn start_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let db = &state.services.db;
    if bisect_dao::is_active(db, cluster_id).await? {
        return Err(BisectError::AlreadyRunning.into());
    }

    let linked = PackageStore::list_linked_artifacts(cluster_id, &state.services.content()).await?;
    let mut hashes: Vec<String> = linked
        .iter()
        .filter(|info| info.content_type == ContentType::Mod && info.enabled)
        .map(|info| info.hash.clone())
        .collect();
    hashes.sort();
    hashes.dedup();
    if hashes.len() < 2 {
        return Err(BisectError::TooFewMods.into());
    }

    bisect_dao::start(db, cluster_id, &hashes).await?;
    tracing::info!(
        cluster_id,
        mods = hashes.len(),
        "started problem mod search"
    );

    let session = load(state, cluster_id)
        .await?
        .ok_or(BisectError::NotRunning)?;
    advance(state, cluster_id, session).await
}

#[tracing::instrument(skip(state))]
pub async fn answer_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    answer: BisectAnswer,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id)
        .await?
        .ok_or(BisectError::NotRunning)?;

    let suspects = session.suspects();
    let kept = plan::narrow(&suspects, &session.testing, answer == BisectAnswer::Broken);
    for index in suspects.difference(&kept) {
        session.cleared[*index] = Some(session.round);
    }
    session.round += 1;
    tracing::info!(
        cluster_id,
        ?answer,
        round = session.round,
        suspects = kept.len(),
        "recorded problem mod search answer"
    );

    advance(state, cluster_id, session).await
}

#[tracing::instrument(skip(state))]
pub async fn skip_bisect_answer(state: &Arc<LauncherState>, cluster_id: i64) -> LauncherResult<()> {
    bisect_dao::set_pending_exit(&state.services.db, cluster_id, BisectExit::None).await?;
    state.services.events.signal(Signal::ClustersChanged);
    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn undo_bisect_answer(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id)
        .await?
        .ok_or(BisectError::NotRunning)?;
    if session.round == 0 {
        return Err(BisectError::NothingToUndo.into());
    }

    session.round -= 1;
    for cleared in &mut session.cleared {
        if *cleared == Some(session.round) {
            *cleared = None;
        }
    }

    advance(state, cluster_id, session).await
}

#[tracing::instrument(skip(state))]
pub async fn finish_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    disable: &[String],
) -> LauncherResult<()> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let ctx = state.services.content();
    let Some(session) = load(state, cluster_id).await? else {
        return Err(BisectError::NotRunning.into());
    };

    for hash in session
        .hashes
        .iter()
        .filter(|h| session.linked.contains_key(*h))
    {
        if let Err(err) = PackageStore::set_artifact_enabled_to(cluster_id, hash, true, &ctx).await
        {
            tracing::warn!(cluster_id, %hash, error = %err, "could not re-enable a mod after the search");
        }
    }

    bisect_dao::end(&state.services.db, cluster_id).await?;

    for hash in disable {
        if let Err(err) =
            oneclient_content::bundles::set_artifact_enabled_to(cluster_id, hash, false, &ctx).await
        {
            tracing::warn!(cluster_id, %hash, error = %err, "could not disable the problem mod");
        }
    }

    tracing::info!(
        cluster_id,
        disabled = disable.len(),
        "finished problem mod search"
    );
    state.services.events.signal(Signal::ClustersChanged);
    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn keep_bisect_mods_on(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    hashes: &[String],
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id)
        .await?
        .ok_or(BisectError::NotRunning)?;

    let off = session.off();
    let targets: ModSet = hashes
        .iter()
        .filter_map(|hash| session.index_of(hash))
        .filter(|index| off.contains(index))
        .collect();
    let edges = edges_from_testing(&session, &targets);
    learn(state, cluster_id, &mut session, edges).await?;

    let note = format!(
        "{} will stay on with the mods being tested. Launch again.",
        session.names(&targets)
    );
    let mut status = advance(state, cluster_id, session).await?;
    bisect_dao::set_retry_note(&state.services.db, cluster_id, &note).await?;
    status.retry_note = Some(note);
    Ok(status)
}

pub(crate) async fn record_bisect_exit(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    crashed: bool,
    missing: Vec<MissingDependency>,
) {
    if crashed && !missing.is_empty() {
        match repair_round(state, cluster_id, &missing).await {
            Ok(true) => return,
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "could not repair the mod search round");
            }
        }
    }

    let exit = if crashed {
        BisectExit::Crashed
    } else {
        BisectExit::Clean
    };
    match bisect_dao::set_pending_exit(&state.services.db, cluster_id, exit).await {
        Ok(true) => state.services.events.signal(Signal::ClustersChanged),
        Ok(false) => {}
        Err(err) => {
            tracing::warn!(cluster_id, error = %err, "could not record the exit for the mod search");
        }
    }
}

async fn repair_round(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    missing: &[MissingDependency],
) -> LauncherResult<bool> {
    let _guard = OPS.lock().await;
    let Some(mut session) = load(state, cluster_id).await? else {
        return Ok(false);
    };
    if session.suspects().is_subset(&session.testing) {
        return Ok(false);
    }

    let graph = dependency_graph(state, &session).await;
    let off = session.off();
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut requesters = ModSet::new();
    let mut unknown_requester = false;
    let mut targets = ModSet::new();

    for found in missing {
        let target = match &found.missing {
            MissingTarget::Mod(id) => graph.provides.get(id).copied(),
            MissingTarget::Class(class) => jar_with_class(state, &session, &off, class).await,
        };
        let Some(target) = target.filter(|index| off.contains(index)) else {
            continue;
        };
        targets.insert(target);

        let requester = found
            .requester
            .as_ref()
            .and_then(|id| graph.provides.get(id).copied())
            .filter(|index| session.testing.contains(index));
        match requester {
            Some(from) => {
                requesters.insert(from);
                edges.push((session.hashes[from].clone(), session.hashes[target].clone()));
            }
            None => {
                unknown_requester = true;
                edges.extend(edges_from_testing(&session, &ModSet::from([target])));
            }
        }
    }

    if !learn(state, cluster_id, &mut session, edges).await? {
        return Ok(false);
    }

    let who = if unknown_requester || requesters.is_empty() {
        "A mod that was on".to_string()
    } else {
        session.names(&requesters)
    };
    let note = format!(
        "That launch failed because {who} needs {}, which was off. It's fixed now, so launch again.",
        session.names(&targets)
    );
    tracing::info!(cluster_id, %note, "repaired a mod search round after a missing dependency");

    advance(state, cluster_id, session).await?;
    bisect_dao::set_retry_note(&state.services.db, cluster_id, &note).await?;
    state.services.events.signal(Signal::ClustersChanged);
    Ok(true)
}

fn edges_from_testing(session: &Session, targets: &ModSet) -> Vec<(String, String)> {
    session
        .testing
        .iter()
        .flat_map(|&from| {
            targets
                .iter()
                .filter(move |&&to| to != from)
                .map(move |&to| (session.hashes[from].clone(), session.hashes[to].clone()))
        })
        .collect()
}

async fn learn(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    session: &mut Session,
    edges: Vec<(String, String)>,
) -> LauncherResult<bool> {
    let fresh: Vec<(String, String)> = edges
        .into_iter()
        .filter(|edge| !session.edges.contains(edge))
        .collect();
    if fresh.is_empty() {
        return Ok(false);
    }
    bisect_dao::add_edges(&state.services.db, cluster_id, &fresh).await?;
    session.edges.extend(fresh);
    Ok(true)
}

async fn jar_with_class(
    state: &Arc<LauncherState>,
    session: &Session,
    candidates: &ModSet,
    class: &str,
) -> Option<usize> {
    let entry = format!("{class}.class");
    for &index in candidates {
        let Some(path) = artifact_path(state, &session.hashes[index]).await else {
            continue;
        };
        if polyio::zip_entry_names(&path)
            .await
            .is_ok_and(|names| names.iter().any(|name| *name == entry))
        {
            return Some(index);
        }
    }
    None
}

async fn artifact_path(state: &Arc<LauncherState>, hash: &str) -> Option<PathBuf> {
    match artifact_dao::get_artifact_by_hash(&state.services.db, hash).await {
        Ok(Some(row)) => artifact_absolute_path(&row.path).ok(),
        _ => None,
    }
}

fn ensure_idle(state: &Arc<LauncherState>, cluster_id: i64) -> Result<(), BisectError> {
    if state.games.is_active(cluster_id) {
        Err(BisectError::GameRunning)
    } else {
        Ok(())
    }
}

async fn load(state: &Arc<LauncherState>, cluster_id: i64) -> LauncherResult<Option<Session>> {
    let db = &state.services.db;
    let Some(row) = bisect_dao::get_session(db, cluster_id).await? else {
        return Ok(None);
    };
    let mods = bisect_dao::list_mods(db, cluster_id).await?;
    let edges = bisect_dao::list_edges(db, cluster_id).await?;
    let linked = PackageStore::list_linked_artifacts(cluster_id, &state.services.content())
        .await?
        .into_iter()
        .filter(|info| info.content_type == ContentType::Mod)
        .map(|info| (info.hash.clone(), info))
        .collect();

    let testing = mods
        .iter()
        .enumerate()
        .filter(|(_, m)| m.testing != 0)
        .map(|(i, _)| i)
        .collect();

    Ok(Some(Session {
        round: row.round,
        pending_exit: row.pending_exit(),
        retry_note: row.retry_note,
        hashes: mods.iter().map(|m| m.hash.clone()).collect(),
        cleared: mods.iter().map(|m| m.cleared_round).collect(),
        testing,
        linked,
        edges,
    }))
}

async fn dependency_graph(state: &Arc<LauncherState>, session: &Session) -> Graph {
    let mut provides: HashMap<String, usize> = HashMap::new();
    let mut requires: Vec<Vec<String>> = Vec::with_capacity(session.hashes.len());

    for hash in &session.hashes {
        let Some(path) = artifact_path(state, hash).await else {
            requires.push(Vec::new());
            continue;
        };
        let deps = read_jar_dependencies(&path).await;
        let index = requires.len();
        for id in deps.provides {
            provides.entry(id).or_insert(index);
        }
        requires.push(deps.requires);
    }

    let mut graph: Vec<Vec<usize>> = requires
        .into_iter()
        .enumerate()
        .map(|(index, ids)| {
            ids.iter()
                .filter_map(|id| provides.get(id).copied())
                .filter(|&target| target != index)
                .collect()
        })
        .collect();

    for (from, to) in &session.edges {
        if let (Some(from), Some(to)) = (session.index_of(from), session.index_of(to))
            && from != to
        {
            graph[from].push(to);
        }
    }

    for edges in &mut graph {
        edges.sort_unstable();
        edges.dedup();
    }

    Graph {
        requires: graph,
        provides,
    }
}

async fn advance(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    mut session: Session,
) -> LauncherResult<BisectStatus> {
    let graph = dependency_graph(state, &session).await.requires;
    let suspects = session.suspects();

    session.testing = match plan::next_round(&graph, &suspects) {
        Some(enabled) => enabled,
        None => plan::closure(&graph, suspects.iter().copied()),
    };
    session.pending_exit = BisectExit::None;
    session.retry_note = None;

    let ctx = state.services.content();
    for (index, hash) in session.hashes.iter().enumerate() {
        let Some(info) = session.linked.get_mut(hash) else {
            continue;
        };
        let want = session.testing.contains(&index);
        if info.enabled == want {
            continue;
        }
        match PackageStore::set_artifact_enabled_to(cluster_id, hash, want, &ctx).await {
            Ok(_) => info.enabled = want,
            Err(err) => {
                tracing::warn!(cluster_id, %hash, error = %err, "could not switch a mod for the search");
            }
        }
    }

    let states: Vec<BisectModState<'_>> = session
        .hashes
        .iter()
        .enumerate()
        .map(|(index, hash)| BisectModState {
            hash,
            cleared_round: session.cleared[index],
            testing: session.testing.contains(&index),
        })
        .collect();
    bisect_dao::save_round(&state.services.db, cluster_id, session.round, &states).await?;

    state.services.events.signal(Signal::ClustersChanged);
    Ok(describe(cluster_id, &session))
}

fn describe(cluster_id: i64, session: &Session) -> BisectStatus {
    let suspects = session.suspects();
    let finished = suspects.is_subset(&session.testing);

    let mods = session
        .hashes
        .iter()
        .enumerate()
        .filter_map(|(index, hash)| {
            let info = session.linked.get(hash)?;
            let testing = session.testing.contains(&index);
            let role = match (suspects.contains(&index), testing) {
                (true, true) => BisectRole::Testing,
                (true, false) => BisectRole::Waiting,
                (false, true) => BisectRole::Helper,
                (false, false) => BisectRole::Cleared,
            };
            Some(BisectMod {
                hash: hash.clone(),
                name: info
                    .display_name
                    .clone()
                    .unwrap_or_else(|| info.file_name.clone()),
                role,
            })
        })
        .collect();

    BisectStatus {
        cluster_id,
        round: session.round,
        pending_exit: session.pending_exit,
        mods,
        finished,
        retry_note: session.retry_note.clone(),
    }
}
