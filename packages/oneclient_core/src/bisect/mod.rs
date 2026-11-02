mod plan;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use oneclient_common::domain::ContentType;
use oneclient_content::packages::store::artifact_absolute_path;
use oneclient_content::packages::types::DependencyKind;
use oneclient_content::packages::{
    JarDependencies, LinkedArtifactInfo, PackageStore, ProviderId, read_cached_package_meta,
    read_jar_dependencies, read_jar_manifest,
};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bisect as bisect_dao;
use oneclient_db::dao::cluster_bisect::BisectModState;
use oneclient_db::models::BisectEdgeKind;
pub use oneclient_db::models::BisectExit;
use oneclient_events::Signal;
use thiserror::Error;

use crate::LauncherResult;
use crate::game::diagnosis::{
    LogFindings, MissingDependency, MissingTarget, crash_report_findings,
};
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

    #[error("That mod is no longer off, so answer the question instead")]
    NoSuggestion,

    #[error("There is no result to confirm yet")]
    NothingToConfirm,

    #[error(
        "Couldn't turn {name} {direction}: {reason}. Close anything using the mod file and try again."
    )]
    SwitchFailed {
        name: String,
        direction: &'static str,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectRole {
    Testing,
    Waiting,
    Helper,
    Cleared,
    /// Found earlier and kept on while looking for a mod it conflicts with
    Found,
}

impl BisectRole {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Testing => "Testing",
            Self::Waiting => "Off this round",
            Self::Helper => "Needed by a tested mod",
            Self::Cleared => "Cleared",
            Self::Found => "Found",
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
    pub suggestion: Option<BisectSuggestion>,
    /// The user said the problem still happens with only the found mods on
    pub confirmed: bool,
    /// Mods are being put back a failure leaves it here until retried
    pub finishing: bool,
    /// Names of mods in the search that left the cluster since it started
    pub removed: Vec<String>,
    /// Names of enabled mods that joined the cluster since it started they are
    /// on for every launch and never tested
    pub added: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectSuggestion {
    pub missing: Vec<String>,
    pub needed_by: Vec<String>,
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
        self.pending_exit != BisectExit::None && !self.confirmed && !self.finishing
    }

    /// The launch after a result asks whether the found mods alone still cause it
    #[must_use]
    pub fn awaiting_confirmation(&self) -> bool {
        self.awaiting_answer() && self.finished
    }

    /// The mods to blame the ones found earlier plus the suspects once only
    /// they are left
    #[must_use]
    pub fn culprits(&self) -> Vec<&BisectMod> {
        self.mods
            .iter()
            .filter(|m| {
                m.role == BisectRole::Found || (self.finished && m.role == BisectRole::Testing)
            })
            .collect()
    }

    #[must_use]
    pub fn mods_changed(&self) -> bool {
        !self.removed.is_empty() || !self.added.is_empty()
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

static JAR_DEPENDENCIES: std::sync::LazyLock<std::sync::Mutex<HashMap<String, JarDependencies>>> =
    std::sync::LazyLock::new(Default::default);

struct Session {
    round: i64,
    pending_exit: BisectExit,
    retry_note: Option<String>,
    confirmed: bool,
    finishing: bool,
    hashes: Vec<String>,
    cleared: Vec<Option<i64>>,
    pinned: Vec<Option<i64>>,
    disable_on_finish: Vec<bool>,
    testing: ModSet,
    linked: HashMap<String, LinkedArtifactInfo>,
    names: HashMap<String, String>,
    edges: Vec<(String, String, BisectEdgeKind)>,
    suggestions: Vec<(Option<String>, String)>,
    removed: Vec<String>,
    added: Vec<String>,
}

impl Session {
    /// Round the current search started at pinning the found mods to look for
    /// a partner makes everything cleared before it a suspect again
    fn phase_round(&self) -> i64 {
        self.pinned
            .iter()
            .flatten()
            .max()
            .map_or(0, |round| round + 1)
    }

    fn suspects(&self) -> ModSet {
        let phase = self.phase_round();
        (0..self.hashes.len())
            .filter(|&i| {
                self.pinned[i].is_none()
                    && self.cleared[i].is_none_or(|round| round < phase)
                    && self.linked.contains_key(&self.hashes[i])
            })
            .collect()
    }

    fn pinned_set(&self) -> ModSet {
        (0..self.hashes.len())
            .filter(|&i| self.pinned[i].is_some() && self.linked.contains_key(&self.hashes[i]))
            .collect()
    }

    fn finished(&self) -> bool {
        self.suspects().is_subset(&self.testing)
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
        let hash = &self.hashes[index];
        self.names
            .get(hash)
            .cloned()
            .unwrap_or_else(|| hash.clone())
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
    firm: Vec<Vec<usize>>,
    /// A jar's own mod ids
    own: HashMap<String, usize>,
    /// Libraries nested inside jars one id is often bundled by several mods
    bundled: HashMap<String, Vec<usize>>,
}

impl Graph {
    /// Only an unambiguous provider when several mods bundle the same library
    /// any of them satisfies it so no single one is needed
    fn provider(&self, id: &str) -> Option<usize> {
        self.own
            .get(id)
            .copied()
            .or_else(|| match self.bundled.get(id) {
                Some(providers) if providers.len() == 1 => Some(providers[0]),
                _ => None,
            })
    }

    fn provider_among(&self, id: &str, among: &ModSet) -> Option<usize> {
        self.own
            .get(id)
            .copied()
            .filter(|index| among.contains(index))
            .or_else(|| {
                self.bundled
                    .get(id)?
                    .iter()
                    .copied()
                    .find(|index| among.contains(index))
            })
    }
}

type Edge = (String, String, BisectEdgeKind);

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

#[tracing::instrument(skip(state, progress))]
pub async fn start_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    progress: impl Fn(&str, u64, u64),
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let db = &state.services.db;
    if bisect_dao::is_active(db, cluster_id).await? {
        return Err(BisectError::AlreadyRunning.into());
    }

    let linked = PackageStore::list_linked_artifacts(cluster_id, &state.services.content()).await?;
    let mut enabled: Vec<&LinkedArtifactInfo> = linked
        .iter()
        .filter(|info| info.content_type == ContentType::Mod && info.enabled)
        .collect();
    enabled.sort_by(|a, b| a.hash.cmp(&b.hash));
    enabled.dedup_by(|a, b| a.hash == b.hash);
    if enabled.len() < 2 {
        return Err(BisectError::TooFewMods.into());
    }

    let total = enabled.len() as u64 + 2;
    for (read, info) in enabled.iter().enumerate() {
        progress("Reading mods", read as u64, total);
        jar_dependencies(state, &info.hash).await;
    }

    progress("Checking dependencies online", total - 2, total);
    let names = mod_names(state, &enabled).await;
    let mods: Vec<(String, String)> = enabled
        .iter()
        .map(|info| {
            let name = names
                .get(&info.hash)
                .cloned()
                .unwrap_or_else(|| info.file_name.clone());
            (info.hash.clone(), name)
        })
        .collect();

    bisect_dao::start(db, cluster_id, &mods).await?;
    tracing::info!(cluster_id, mods = mods.len(), "started problem mod search");

    let listed = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        listed_dependency_edges(state, &enabled),
    )
    .await
    .unwrap_or_else(|_| {
        tracing::warn!(cluster_id, "reading the listed mod dependencies took too long");
        Vec::new()
    });
    if !listed.is_empty()
        && let Err(err) = bisect_dao::add_edges(db, cluster_id, &listed).await
    {
        tracing::warn!(cluster_id, error = %err, "could not save the listed mod dependencies");
    }

    progress("Switching mods for the first round", total - 1, total);
    let started = match load(state, cluster_id).await {
        Ok(Some(session)) => advance(state, cluster_id, session).await,
        Ok(None) => Err(BisectError::NotRunning.into()),
        Err(err) => Err(err),
    };
    // A first round that could not be set up leaves nothing worth resuming
    if started.is_err()
        && let Err(err) = bisect_dao::end(db, cluster_id).await
    {
        tracing::warn!(cluster_id, error = %err, "could not drop a mod search that failed to start");
    }
    started
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
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    bisect_dao::clear_suggestions(&state.services.db, cluster_id).await?;
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
    if session.confirmed {
        bisect_dao::set_confirmed(&state.services.db, cluster_id, false).await?;
        state.services.events.signal(Signal::ClustersChanged);
        session.confirmed = false;
        session.pending_exit = BisectExit::None;
        return Ok(describe(cluster_id, &session));
    }
    if session.round == 0 {
        return Err(BisectError::NothingToUndo.into());
    }

    session.round -= 1;
    let round = Some(session.round);
    for cleared in &mut session.cleared {
        if *cleared == round {
            *cleared = None;
        }
    }
    for pinned in &mut session.pinned {
        if *pinned == round {
            *pinned = None;
        }
    }

    advance(state, cluster_id, session).await
}

/// Answers the launch after a result where only the found mods and what they
/// need were on
#[tracing::instrument(skip(state))]
pub async fn confirm_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    reproduced: bool,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let db = &state.services.db;
    let mut session = load(state, cluster_id)
        .await?
        .ok_or(BisectError::NotRunning)?;
    if !session.finished() || session.confirmed {
        return Err(BisectError::NothingToConfirm.into());
    }

    if reproduced {
        bisect_dao::set_confirmed(db, cluster_id, true).await?;
        tracing::info!(cluster_id, "confirmed the problem mod");
        state.services.events.signal(Signal::ClustersChanged);
        session.confirmed = true;
        session.pending_exit = BisectExit::None;
        return Ok(describe(cluster_id, &session));
    }

    let found: ModSet = session
        .suspects()
        .union(&session.pinned_set())
        .copied()
        .collect();
    let found_names = session.names(&found);
    let candidates = (0..session.hashes.len())
        .filter(|i| !found.contains(i) && session.linked.contains_key(&session.hashes[*i]))
        .count();
    if candidates == 0 {
        bisect_dao::set_confirmed(db, cluster_id, true).await?;
        let note = format!(
            "The problem didn't happen with only {found_names} on. It may only happen sometimes, or need a mod that wasn't part of the search."
        );
        bisect_dao::set_retry_note(db, cluster_id, &note).await?;
        state.services.events.signal(Signal::ClustersChanged);
        session.confirmed = true;
        session.pending_exit = BisectExit::None;
        session.retry_note = Some(note);
        return Ok(describe(cluster_id, &session));
    }

    for &index in &found {
        if session.pinned[index].is_none() {
            session.pinned[index] = Some(session.round);
        }
    }
    session.round += 1;
    tracing::info!(
        cluster_id,
        candidates,
        "looking for a mod the found one conflicts with"
    );

    let note = format!(
        "{found_names} alone didn't cause it, so now looking for a mod that causes it together with them. They stay on for every launch."
    );
    let mut status = advance(state, cluster_id, session).await?;
    bisect_dao::set_retry_note(db, cluster_id, &note).await?;
    status.retry_note = Some(note);
    Ok(status)
}

/// Starts over with the mods the cluster has now after mods changed under a
/// running search
#[tracing::instrument(skip(state))]
pub async fn restart_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<BisectStatus> {
    finish_bisect(state, cluster_id, &[]).await?;
    start_bisect(state, cluster_id, |_, _, _| {}).await
}

#[tracing::instrument(skip(state))]
pub async fn finish_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    disable: &[String],
) -> LauncherResult<()> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let Some(session) = load(state, cluster_id).await? else {
        return Err(BisectError::NotRunning.into());
    };

    // A retry after a failed restore keeps the choice made the first time
    if !session.finishing {
        let disable: Vec<String> = disable
            .iter()
            .filter(|hash| session.index_of(hash).is_some())
            .cloned()
            .collect();
        bisect_dao::begin_finish(&state.services.db, cluster_id, &disable).await?;
    }

    complete_finish(state, cluster_id).await
}

/// Each step is safe to repeat so a finish cut short by a crash or a locked
/// file is simply run again
async fn complete_finish(state: &Arc<LauncherState>, cluster_id: i64) -> LauncherResult<()> {
    let ctx = state.services.content();
    let Some(session) = load(state, cluster_id).await? else {
        return Ok(());
    };

    let mut failure: Option<BisectError> = None;
    for (index, hash) in session.hashes.iter().enumerate() {
        if !session.linked.contains_key(hash) {
            continue;
        }
        let disable = session.disable_on_finish[index];
        let result = if disable {
            oneclient_content::bundles::set_artifact_enabled_to(cluster_id, hash, false, &ctx)
                .await
                .map(|_| ())
        } else {
            PackageStore::set_artifact_enabled_to(cluster_id, hash, true, &ctx)
                .await
                .map(|_| ())
        };
        if let Err(err) = result {
            tracing::warn!(cluster_id, %hash, error = %err, "could not restore a mod after the search");
            failure.get_or_insert(BisectError::SwitchFailed {
                name: session.name(index),
                direction: if disable { "off" } else { "back on" },
                reason: err.to_string(),
            });
        }
    }

    if let Some(err) = failure {
        state.services.events.signal(Signal::ClustersChanged);
        return Err(err.into());
    }

    bisect_dao::end(&state.services.db, cluster_id).await?;
    tracing::info!(
        cluster_id,
        disabled = session.disable_on_finish.iter().filter(|d| **d).count(),
        "finished problem mod search"
    );
    state.services.events.signal(Signal::ClustersChanged);
    Ok(())
}

/// Completes any finish the launcher was closed in the middle of
pub async fn resume_unfinished_bisects(state: &Arc<LauncherState>) {
    let clusters = match bisect_dao::list_finishing(&state.services.db).await {
        Ok(clusters) => clusters,
        Err(err) => {
            tracing::warn!(error = %err, "could not look for unfinished mod searches");
            return;
        }
    };
    for cluster_id in clusters {
        let _guard = OPS.lock().await;
        if ensure_idle(state, cluster_id).is_err() {
            continue;
        }
        if let Err(err) = complete_finish(state, cluster_id).await {
            tracing::warn!(cluster_id, error = %err, "could not finish restoring mods after a search");
        }
    }
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
    let edges = edges_from_testing(&session, &targets, BisectEdgeKind::Guessed);
    learn(state, cluster_id, &mut session, edges).await?;

    let kept = if targets.len() > 3 {
        format!("{} mods", targets.len())
    } else {
        session.names(&targets)
    };
    let note = format!("{kept} will stay on with the mods being tested. Launch again.");
    let mut status = advance(state, cluster_id, session).await?;
    bisect_dao::set_retry_note(&state.services.db, cluster_id, &note).await?;
    status.retry_note = Some(note);
    Ok(status)
}

#[tracing::instrument(skip(state))]
pub async fn accept_bisect_suggestion(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id)
        .await?
        .ok_or(BisectError::NotRunning)?;

    let off = session.off();
    let mut edges: Vec<Edge> = Vec::new();
    let mut targets = ModSet::new();
    let mut requesters = ModSet::new();
    for (from, to) in session.suggestions.clone() {
        let Some(to) = session.index_of(&to).filter(|index| off.contains(index)) else {
            continue;
        };
        targets.insert(to);
        let from = from
            .and_then(|from| session.index_of(&from))
            .filter(|index| session.testing.contains(index));
        match from {
            Some(from) => {
                requesters.insert(from);
                edges.push(edge(&session, from, to, BisectEdgeKind::Known));
            }
            None => edges.extend(edges_from_testing(
                &session,
                &ModSet::from([to]),
                BisectEdgeKind::Guessed,
            )),
        }
    }

    if targets.is_empty() {
        bisect_dao::clear_suggestions(&state.services.db, cluster_id).await?;
        state.services.events.signal(Signal::ClustersChanged);
        return Err(BisectError::NoSuggestion.into());
    }

    learn(state, cluster_id, &mut session, edges).await?;

    let with = if requesters.is_empty() {
        "the mods being tested".to_string()
    } else {
        session.names(&requesters)
    };
    let note = format!(
        "{} will stay on with {with}. Launch again.",
        session.names(&targets)
    );
    let mut status = advance(state, cluster_id, session).await?;
    bisect_dao::set_retry_note(&state.services.db, cluster_id, &note).await?;
    status.retry_note = Some(note);
    Ok(status)
}

#[tracing::instrument(skip(state))]
pub async fn decline_bisect_suggestion(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<()> {
    let _guard = OPS.lock().await;
    ensure_idle(state, cluster_id)?;
    bisect_dao::clear_suggestions(&state.services.db, cluster_id).await?;
    state.services.events.signal(Signal::ClustersChanged);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GameExit {
    Clean,
    Crashed,
    /// The launcher lost the process or recovered the session afterwards so
    /// it cannot say how the game ended
    Unknown,
}

pub(crate) async fn record_bisect_exit(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    exit: GameExit,
    findings: LogFindings,
    game_dir: &Path,
    started_at: SystemTime,
) {
    if !matches!(
        bisect_dao::is_active(&state.services.db, cluster_id).await,
        Ok(true)
    ) {
        return;
    }

    if exit == GameExit::Unknown {
        let db = &state.services.db;
        let note = "OneClient couldn't tell how the game ended, so this launch doesn't count. Launch again to test this round.";
        let recorded = async {
            bisect_dao::clear_suggestions(db, cluster_id).await?;
            bisect_dao::set_pending_exit(db, cluster_id, BisectExit::None).await?;
            bisect_dao::set_retry_note(db, cluster_id, note).await
        };
        if let Err(err) = recorded.await {
            tracing::warn!(cluster_id, error = %err, "could not record an unclear exit for the mod search");
        }
        state.services.events.signal(Signal::ClustersChanged);
        return;
    }
    let crashed = exit == GameExit::Crashed;

    let mut sure = findings.sure;
    let mut classes = findings.classes;
    if crashed {
        let report = crash_report_findings(game_dir, started_at).await;
        merge_findings(&mut sure, report.sure);
        merge_findings(&mut classes, report.classes);
    }

    // A clean exit means nothing was missing badly enough to matter, whatever
    // the loader warned about along the way
    if crashed && !sure.is_empty() {
        match repair_round(state, cluster_id, &sure).await {
            Ok(true) => return,
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "could not repair the mod search round");
            }
        }
    }

    if crashed && !classes.is_empty() {
        match suggest(state, cluster_id, &classes).await {
            Ok(true) => return,
            Ok(false) => {}
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "could not suggest a missing mod");
            }
        }
    }

    if let Err(err) = bisect_dao::clear_suggestions(&state.services.db, cluster_id).await {
        tracing::warn!(cluster_id, error = %err, "could not clear an old missing mod suggestion");
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

fn merge_findings(into: &mut Vec<MissingDependency>, more: Vec<MissingDependency>) {
    for found in more {
        let seen = into
            .iter_mut()
            .find(|seen| seen.missing == found.missing && seen.requester == found.requester);
        match seen {
            Some(seen) if seen.frames.is_empty() => seen.frames = found.frames,
            Some(_) => {}
            None => into.push(found),
        }
    }
}

async fn repair_round(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    sure: &[MissingDependency],
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
    let mut edges: Vec<Edge> = Vec::new();
    let mut requesters = ModSet::new();
    let mut unknown_requester = false;
    let mut targets = ModSet::new();

    for found in sure {
        let MissingTarget::Mod(id) = &found.missing else {
            continue;
        };
        let Some(target) = graph.provider_among(id, &off) else {
            continue;
        };
        targets.insert(target);

        let requester = found
            .requester
            .as_ref()
            .and_then(|id| graph.provider(id))
            .filter(|index| session.testing.contains(index));
        match requester {
            Some(from) => {
                requesters.insert(from);
                edges.push(edge(&session, from, target, BisectEdgeKind::Known));
            }
            None => {
                unknown_requester = true;
                edges.extend(edges_from_testing(
                    &session,
                    &ModSet::from([target]),
                    BisectEdgeKind::Guessed,
                ));
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

async fn suggest(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    classes: &[MissingDependency],
) -> LauncherResult<bool> {
    let _guard = OPS.lock().await;
    let Some(session) = load(state, cluster_id).await? else {
        return Ok(false);
    };
    if session.suspects().is_subset(&session.testing) {
        return Ok(false);
    }

    let off = session.off();
    let mut jars = JarEntries::default();
    let mut suggestions: Vec<(Option<String>, String)> = Vec::new();

    for found in classes {
        let MissingTarget::Class(class) = &found.missing else {
            continue;
        };
        let Some(target) = jars.find(state, &session, &off, class).await else {
            continue;
        };

        let mut requester = None;
        for frame in &found.frames {
            if let Some(from) = jars.find(state, &session, &session.testing, frame).await {
                requester = Some(session.hashes[from].clone());
                break;
            }
        }

        let suggestion = (requester, session.hashes[target].clone());
        if !suggestions.contains(&suggestion) {
            suggestions.push(suggestion);
        }
    }

    if suggestions.is_empty() {
        return Ok(false);
    }

    bisect_dao::set_suggestions(
        &state.services.db,
        cluster_id,
        &suggestions,
        BisectExit::Crashed,
    )
    .await?;
    tracing::info!(
        cluster_id,
        count = suggestions.len(),
        "asking whether a missing mod caused the crash"
    );
    state.services.events.signal(Signal::ClustersChanged);
    Ok(true)
}

async fn listed_dependency_edges(
    state: &Arc<LauncherState>,
    mods: &[&LinkedArtifactInfo],
) -> Vec<Edge> {
    let ctx = state.services.content();
    let mut by_provider: HashMap<ProviderId, Vec<(&str, &str, &str)>> = HashMap::new();
    for info in mods {
        if let (Some(provider), Some(project), Some(version)) =
            (info.provider, &info.project_id, &info.version_id)
            && provider != ProviderId::Local
        {
            by_provider.entry(provider).or_default().push((
                info.hash.as_str(),
                project.as_str(),
                version.as_str(),
            ));
        }
    }

    let mut edges: Vec<Edge> = Vec::new();
    for (provider, listed) in by_provider {
        let Ok(client) = ctx.providers.get(provider) else {
            continue;
        };
        let version_ids: Vec<String> = listed.iter().map(|(_, _, v)| (*v).to_string()).collect();
        let mut required: Vec<(String, String)> = Vec::new();
        for chunk in version_ids.chunks(100) {
            match client.get_versions(chunk, &ctx).await {
                Ok(versions) => required.extend(versions.into_iter().flat_map(|version| {
                    version
                        .dependencies
                        .into_iter()
                        .filter(|d| d.kind == DependencyKind::Required)
                        .filter_map(|d| d.project_id)
                        .map(move |project| (version.version_id.clone(), project))
                })),
                Err(err) => {
                    tracing::warn!(?provider, error = %err, "could not read the listed dependencies for the mod search");
                }
            }
        }
        for edge in required_edges(&listed, &required) {
            if !edges.contains(&edge) {
                edges.push(edge);
            }
        }
    }
    tracing::info!(
        edges = edges.len(),
        "read the dependencies mods list on their provider"
    );
    edges
}

fn required_edges(mods: &[(&str, &str, &str)], required: &[(String, String)]) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (version, project) in required {
        for (from, _, _) in mods.iter().filter(|(_, _, v)| v == version) {
            for (to, _, _) in mods.iter().filter(|(h, p, _)| p == project && h != from) {
                edges.push((from.to_string(), to.to_string(), BisectEdgeKind::Known));
            }
        }
    }
    edges
}

fn edge(session: &Session, from: usize, to: usize, kind: BisectEdgeKind) -> Edge {
    (
        session.hashes[from].clone(),
        session.hashes[to].clone(),
        kind,
    )
}

fn edges_from_testing(session: &Session, targets: &ModSet, kind: BisectEdgeKind) -> Vec<Edge> {
    session
        .testing
        .iter()
        .flat_map(|&from| {
            targets
                .iter()
                .filter(move |&&to| to != from)
                .map(move |&to| edge(session, from, to, kind))
        })
        .collect()
}

async fn learn(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    session: &mut Session,
    edges: Vec<Edge>,
) -> LauncherResult<bool> {
    let covered = |list: &[Edge], (from, to, kind): &Edge| {
        list.iter()
            .any(|(f, t, k)| f == from && t == to && k <= kind)
    };

    let mut fresh: Vec<Edge> = Vec::new();
    for edge in edges {
        if !covered(&session.edges, &edge) && !covered(&fresh, &edge) {
            fresh.push(edge);
        }
    }
    if fresh.is_empty() {
        return Ok(false);
    }

    bisect_dao::add_edges(&state.services.db, cluster_id, &fresh).await?;
    for (from, to, kind) in fresh {
        match session
            .edges
            .iter_mut()
            .find(|(f, t, _)| *f == from && *t == to)
        {
            Some(existing) => existing.2 = existing.2.min(kind),
            None => session.edges.push((from, to, kind)),
        }
    }
    Ok(true)
}

#[derive(Default)]
struct JarEntries {
    names: HashMap<usize, HashSet<String>>,
}

impl JarEntries {
    async fn find(
        &mut self,
        state: &Arc<LauncherState>,
        session: &Session,
        candidates: &ModSet,
        class: &str,
    ) -> Option<usize> {
        let entry = format!("{class}.class");
        for &index in candidates {
            if !self.names.contains_key(&index) {
                let names = match artifact_path(state, &session.hashes[index]).await {
                    Some(path) => polyio::zip_entry_names(&path)
                        .await
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                    None => HashSet::new(),
                };
                self.names.insert(index, names);
            }
            if self
                .names
                .get(&index)
                .is_some_and(|names| names.contains(&entry))
            {
                return Some(index);
            }
        }
        None
    }
}

async fn jar_dependencies(state: &Arc<LauncherState>, hash: &str) -> Option<JarDependencies> {
    if let Some(known) = JAR_DEPENDENCIES
        .lock()
        .ok()
        .and_then(|cache| cache.get(hash).cloned())
    {
        return Some(known);
    }

    let deps = read_jar_dependencies(&artifact_path(state, hash).await?).await;
    if deps != JarDependencies::default()
        && let Ok(mut cache) = JAR_DEPENDENCIES.lock()
    {
        cache.insert(hash.to_string(), deps.clone());
    }
    Some(deps)
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
    let suggestions = bisect_dao::list_suggestions(db, cluster_id).await?;
    let linked: HashMap<String, LinkedArtifactInfo> =
        PackageStore::list_linked_artifacts(cluster_id, &state.services.content())
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

    // Named once when the search starts sessions from before names were kept
    // are filled in the first time they load
    let mut names: HashMap<String, String> = mods
        .iter()
        .filter_map(|m| Some((m.hash.clone(), m.name.clone()?)))
        .collect();
    let unnamed: Vec<&LinkedArtifactInfo> = mods
        .iter()
        .filter(|m| m.name.is_none())
        .filter_map(|m| linked.get(&m.hash))
        .collect();
    if !unnamed.is_empty() {
        let found: Vec<(String, String)> = mod_names(state, &unnamed).await.into_iter().collect();
        if let Err(err) = bisect_dao::set_names(db, cluster_id, &found).await {
            tracing::warn!(cluster_id, error = %err, "could not save mod names for the search");
        }
        names.extend(found);
    }

    let in_search: HashSet<&str> = mods.iter().map(|m| m.hash.as_str()).collect();
    let removed = mods
        .iter()
        .filter(|m| !linked.contains_key(&m.hash))
        .map(|m| {
            names
                .get(&m.hash)
                .cloned()
                .unwrap_or_else(|| m.hash.clone())
        })
        .collect();
    let joined: Vec<&LinkedArtifactInfo> = linked
        .values()
        .filter(|info| info.enabled && !in_search.contains(info.hash.as_str()))
        .collect();
    let added = if joined.is_empty() {
        Vec::new()
    } else {
        let joined_names = mod_names(state, &joined).await;
        let mut added: Vec<String> = joined
            .iter()
            .map(|info| {
                joined_names
                    .get(&info.hash)
                    .cloned()
                    .unwrap_or_else(|| info.file_name.clone())
            })
            .collect();
        added.sort();
        added
    };

    Ok(Some(Session {
        round: row.round,
        pending_exit: row.pending_exit(),
        retry_note: row.retry_note,
        confirmed: row.confirmed != 0,
        finishing: row.finishing != 0,
        hashes: mods.iter().map(|m| m.hash.clone()).collect(),
        cleared: mods.iter().map(|m| m.cleared_round).collect(),
        pinned: mods.iter().map(|m| m.pinned_round).collect(),
        disable_on_finish: mods.iter().map(|m| m.disable_on_finish != 0).collect(),
        testing,
        linked,
        names,
        edges,
        suggestions,
        removed,
        added,
    }))
}

/// Named the way the mods page names them a linked artifact only carries its
/// release title which is often just a version
async fn mod_names(
    state: &Arc<LauncherState>,
    infos: &[&LinkedArtifactInfo],
) -> HashMap<String, String> {
    let ctx = state.services.content();
    let mut by_provider: HashMap<ProviderId, Vec<String>> = HashMap::new();
    for info in infos {
        let (provider, id) = match (info.provider, &info.project_id) {
            (Some(provider), Some(project_id)) if provider != ProviderId::Local => {
                (provider, project_id.clone())
            }
            _ => (ProviderId::Local, info.hash.clone()),
        };
        by_provider.entry(provider).or_default().push(id);
    }

    let mut project_names: HashMap<(ProviderId, String), String> = HashMap::new();
    for (provider, ids) in by_provider {
        for (id, meta) in read_cached_package_meta(&ctx, provider, &ids).await {
            if !meta.name.trim().is_empty() {
                project_names.insert((provider, id), meta.name);
            }
        }
    }

    let mut names = HashMap::new();
    for info in infos {
        let hash = &info.hash;
        let key = match (info.provider, &info.project_id) {
            (Some(provider), Some(project_id)) if provider != ProviderId::Local => {
                (provider, project_id.clone())
            }
            _ => (ProviderId::Local, hash.clone()),
        };
        let name = match project_names.get(&key).cloned() {
            Some(name) => name,
            None => match artifact_path(state, hash).await {
                Some(path) => read_jar_manifest(&path).await.name,
                None => None,
            }
            .unwrap_or_else(|| info.file_name.clone()),
        };
        names.insert(hash.clone(), name);
    }
    names
}

async fn dependency_graph(state: &Arc<LauncherState>, session: &Session) -> Graph {
    let mut own: HashMap<String, usize> = HashMap::new();
    let mut bundled: HashMap<String, Vec<usize>> = HashMap::new();
    let mut requires: Vec<Vec<String>> = Vec::with_capacity(session.hashes.len());

    for hash in &session.hashes {
        let Some(deps) = jar_dependencies(state, hash).await else {
            requires.push(Vec::new());
            continue;
        };
        let index = requires.len();
        for id in deps.provides {
            own.entry(id).or_insert(index);
        }
        for id in deps.bundled {
            bundled.entry(id).or_default().push(index);
        }
        requires.push(deps.requires);
    }

    let lookup = Graph {
        requires: Vec::new(),
        firm: Vec::new(),
        own,
        bundled,
    };
    let base: Vec<Vec<usize>> = requires
        .into_iter()
        .enumerate()
        .map(|(index, ids)| {
            ids.iter()
                .filter_map(|id| lookup.provider(id))
                .filter(|&target| target != index)
                .collect()
        })
        .collect();

    let mut all = base.clone();
    let mut firm = base;
    for (from, to, kind) in &session.edges {
        if let (Some(from), Some(to)) = (session.index_of(from), session.index_of(to))
            && from != to
        {
            all[from].push(to);
            if *kind == BisectEdgeKind::Known {
                firm[from].push(to);
            }
        }
    }

    for edges in all.iter_mut().chain(firm.iter_mut()) {
        edges.sort_unstable();
        edges.dedup();
    }

    Graph {
        requires: all,
        firm,
        ..lookup
    }
}

async fn advance(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    mut session: Session,
) -> LauncherResult<BisectStatus> {
    let graph = dependency_graph(state, &session).await;
    let suspects = session.suspects();

    // Mods found earlier stay on so this round looks for what they need to
    // break together with
    let kept = plan::closure(&graph.firm, session.pinned_set());
    let split = plan::next_round(&graph.requires, &suspects)
        .or_else(|| plan::next_round(&graph.firm, &suspects))
        .unwrap_or_else(|| plan::closure(&graph.firm, suspects.iter().copied()));
    session.testing = split.union(&kept).copied().collect();
    session.pending_exit = BisectExit::None;
    session.retry_note = None;
    session.suggestions.clear();
    session.confirmed = false;

    let switches: Vec<(usize, bool)> = session
        .hashes
        .iter()
        .enumerate()
        .filter_map(|(index, hash)| {
            let want = session.testing.contains(&index);
            let info = session.linked.get(hash)?;
            (info.enabled != want).then_some((index, want))
        })
        .collect();

    // A round only counts if every mod in it really switched otherwise the
    // answer would be about a different set of mods than the one recorded
    let ctx = state.services.content();
    let mut done: Vec<(usize, bool)> = Vec::new();
    for &(index, want) in &switches {
        let hash = &session.hashes[index];
        if let Err(err) = PackageStore::set_artifact_enabled_to(cluster_id, hash, want, &ctx).await
        {
            tracing::warn!(cluster_id, %hash, error = %err, "could not switch a mod for the search");
            for &(index, want) in done.iter().rev() {
                let hash = &session.hashes[index];
                if let Err(err) =
                    PackageStore::set_artifact_enabled_to(cluster_id, hash, !want, &ctx).await
                {
                    tracing::warn!(cluster_id, %hash, error = %err, "could not undo a switch for the search");
                }
            }
            state.services.events.signal(Signal::ClustersChanged);
            return Err(BisectError::SwitchFailed {
                name: session.name(index),
                direction: if want { "on" } else { "off" },
                reason: err.to_string(),
            }
            .into());
        }
        done.push((index, want));
    }
    for (index, want) in done {
        if let Some(info) = session.linked.get_mut(&session.hashes[index]) {
            info.enabled = want;
        }
    }

    let states: Vec<BisectModState<'_>> = session
        .hashes
        .iter()
        .enumerate()
        .map(|(index, hash)| BisectModState {
            hash,
            cleared_round: session.cleared[index],
            pinned_round: session.pinned[index],
            testing: session.testing.contains(&index),
        })
        .collect();
    bisect_dao::save_round(
        &state.services.db,
        cluster_id,
        session.round,
        false,
        &states,
    )
    .await?;

    state.services.events.signal(Signal::ClustersChanged);
    Ok(describe(cluster_id, &session))
}

fn describe(cluster_id: i64, session: &Session) -> BisectStatus {
    let suspects = session.suspects();
    let finished = suspects.is_subset(&session.testing);
    let pinned = session.pinned_set();

    let mods = session
        .hashes
        .iter()
        .enumerate()
        .filter_map(|(index, hash)| {
            session.linked.get(hash)?;
            let testing = session.testing.contains(&index);
            let role = match (suspects.contains(&index), testing) {
                _ if pinned.contains(&index) => BisectRole::Found,
                (true, true) => BisectRole::Testing,
                (true, false) => BisectRole::Waiting,
                (false, true) => BisectRole::Helper,
                (false, false) => BisectRole::Cleared,
            };
            Some(BisectMod {
                hash: hash.clone(),
                name: session.name(index),
                role,
            })
        })
        .collect();

    let suggestion = (!session.suggestions.is_empty()).then(|| {
        let mut missing: Vec<String> = Vec::new();
        let mut needed_by: Vec<String> = Vec::new();
        for (from, to) in &session.suggestions {
            if let Some(name) = session.index_of(to).map(|index| session.name(index))
                && !missing.contains(&name)
            {
                missing.push(name);
            }
            if let Some(name) = from
                .as_deref()
                .and_then(|from| session.index_of(from))
                .map(|index| session.name(index))
                && !needed_by.contains(&name)
            {
                needed_by.push(name);
            }
        }
        BisectSuggestion { missing, needed_by }
    });

    BisectStatus {
        cluster_id,
        round: session.round,
        pending_exit: session.pending_exit,
        mods,
        finished,
        retry_note: session.retry_note.clone(),
        suggestion,
        confirmed: session.confirmed,
        finishing: session.finishing,
        removed: session.removed.clone(),
        added: session.added.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listed_dependency_links_to_the_mod_with_that_project() {
        let mods = [
            ("chattweaks", "8LNT47ZH", "v1"),
            ("oneconfig", "AibBIVmj", "v2"),
            ("fabric-api", "P7dR8mSH", "v3"),
        ];
        let required = vec![
            ("v1".to_string(), "AibBIVmj".to_string()),
            ("v1".to_string(), "P7dR8mSH".to_string()),
            ("v1".to_string(), "not-installed".to_string()),
        ];

        let edges = required_edges(&mods, &required);

        assert_eq!(
            edges,
            vec![
                (
                    "chattweaks".to_string(),
                    "oneconfig".to_string(),
                    BisectEdgeKind::Known
                ),
                (
                    "chattweaks".to_string(),
                    "fabric-api".to_string(),
                    BisectEdgeKind::Known
                ),
            ]
        );
    }

    fn graph(own: &[(&str, usize)], bundled: &[(&str, &[usize])]) -> Graph {
        Graph {
            requires: Vec::new(),
            firm: Vec::new(),
            own: own.iter().map(|(id, i)| ((*id).to_string(), *i)).collect(),
            bundled: bundled
                .iter()
                .map(|(id, by)| ((*id).to_string(), by.to_vec()))
                .collect(),
        }
    }

    #[test]
    fn a_mod_of_its_own_wins_over_copies_bundled_elsewhere() {
        let graph = graph(&[("cloth-config", 4)], &[("cloth-config", &[1, 2])]);
        assert_eq!(graph.provider("cloth-config"), Some(4));
    }

    #[test]
    fn a_library_bundled_by_one_mod_points_at_it() {
        let graph = graph(&[], &[("fabric-api-base", &[3])]);
        assert_eq!(graph.provider("fabric-api-base"), Some(3));
    }

    #[test]
    fn a_library_bundled_by_several_mods_needs_none_of_them() {
        let graph = graph(&[], &[("cloth-config", &[1, 2])]);
        assert_eq!(graph.provider("cloth-config"), None);
    }

    #[test]
    fn a_missing_library_is_looked_for_among_the_mods_that_were_off() {
        let graph = graph(&[], &[("cloth-config", &[1, 2])]);
        assert_eq!(
            graph.provider_among("cloth-config", &ModSet::from([2, 5])),
            Some(2)
        );
        assert_eq!(
            graph.provider_among("cloth-config", &ModSet::from([5])),
            None
        );
    }
}
