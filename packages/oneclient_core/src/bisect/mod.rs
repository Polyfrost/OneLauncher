mod imcs;
mod meta;
mod resolve;
mod version;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use oneclient_common::domain::ContentType;
use oneclient_content::packages::store::artifact_absolute_path;
use oneclient_content::packages::types::DependencyKind;
use oneclient_content::packages::{
    LinkedArtifactInfo, PackageStore, ProviderId, read_cached_package_meta, read_jar_manifest,
};
use oneclient_db::dao::artifact as artifact_dao;
use oneclient_db::dao::cluster_bisect as bisect_dao;
use oneclient_db::dao::cluster_bisect::BisectModState;
use oneclient_db::models::BisectEdgeKind;
pub use oneclient_db::models::BisectExit;
use oneclient_events::Signal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::LauncherResult;
use crate::game::diagnosis::{LogFindings, MissingDependency, MissingTarget, crash_report_findings};
use crate::state::LauncherState;
use imcs::{Outcome, Plan, State, Test};
pub use imcs::PlanKind as BisectPlanKind;
use meta::{ClassIndex, ModMeta};
use resolve::{Catalog, Set};
use version::Predicate;

#[derive(Debug, Error)]
pub enum BisectError {
    #[error("Close the game before changing which mods are being tested")]
    GameRunning,

    #[error("A problem mod search is already running for this cluster")]
    AlreadyRunning,

    #[error("A problem mod search is already running in {name}. Finish or stop it there first")]
    RunningElsewhere { name: String },

    #[error("No problem mod search is running for this cluster")]
    NotRunning,

    #[error("At least two enabled mods are needed to search for a problem mod")]
    TooFewMods,

    #[error("There is no earlier answer to undo")]
    NothingToUndo,

    #[error("The search isn't waiting for an answer right now")]
    NothingToAnswer,

    #[error("There are no mods left to search for another problem")]
    CannotContinue,

    #[error("The search is not stuck, so there is nothing to look into")]
    NotHalted,

    #[error("That mod is no longer off, so answer the question instead")]
    NoSuggestion,

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
    Found,
    Problem,
    Required,
    KeptOff,
    Omitted,
    Unresolvable,
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
            Self::Problem => "Problem found earlier",
            Self::Required => "Always on",
            Self::KeptOff => "Kept off",
            Self::Omitted => "Left out",
            Self::Unresolvable => "Can't load",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BisectChoice {
    Search,
    Required,
    KeptOff,
    Omitted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectStage {
    Testing,
    Verifying,
    Halted,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectMod {
    pub hash: String,
    pub name: String,
    pub role: BisectRole,
    pub choice: BisectChoice,
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeMod {
    pub hash: String,
    pub name: String,
    pub also: Vec<String>,
    pub maybe: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConflictReport {
    pub mods: Vec<CascadeMod>,
    pub if_all_also: Vec<String>,
    pub if_all_maybe: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvableMod {
    pub hash: String,
    pub name: String,
    pub missing: Vec<String>,
    pub blocks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenDependency {
    pub from: String,
    pub to: String,
    pub classes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectHistoryEntry {
    pub round: u32,
    pub iteration: u32,
    pub step: u32,
    pub kind: BisectPlanKind,
    pub answer: BisectAnswer,
    pub tested: Vec<String>,
    pub enabled: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectSuggestion {
    pub missing: Vec<String>,
    pub needed_by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BisectStatus {
    pub cluster_id: i64,
    pub stage: BisectStage,
    pub round: u32,
    pub iteration: u32,
    pub tests_this_round: usize,
    pub estimate: u32,
    pub pending_exit: BisectExit,
    pub mods: Vec<BisectMod>,
    pub current: ConflictReport,
    pub earlier: Vec<ConflictReport>,
    pub pending: Vec<String>,
    pub removed: Vec<String>,
    pub can_undo: bool,
    pub can_continue: bool,
    pub groups: Option<(Vec<String>, Vec<String>)>,
    pub can_look_for_hidden: bool,
    pub hidden: Vec<HiddenDependency>,
    pub unresolvable: Vec<UnresolvableMod>,
    pub history: Vec<BisectHistoryEntry>,
    pub resolution_problem: Option<String>,
    pub retry_note: Option<String>,
    pub suggestion: Option<BisectSuggestion>,
    pub finishing: bool,
}

impl BisectStatus {
    #[must_use]
    pub fn finished(&self) -> bool {
        self.stage == BisectStage::Complete
    }

    #[must_use]
    pub fn awaiting_answer(&self) -> bool {
        self.pending_exit != BisectExit::None
            && !self.finishing
            && matches!(self.stage, BisectStage::Testing | BisectStage::Verifying)
    }

    #[must_use]
    pub fn count(&self, role: BisectRole) -> usize {
        self.mods.iter().filter(|m| m.role == role).count()
    }

    #[must_use]
    pub fn culprits(&self) -> Vec<&CascadeMod> {
        self.current.mods.iter().collect()
    }

    pub fn all_found(&self) -> impl Iterator<Item = &CascadeMod> {
        self.earlier
            .iter()
            .chain([&self.current])
            .flat_map(|report| &report.mods)
    }

    pub fn off_this_round(&self) -> impl Iterator<Item = &BisectMod> {
        self.mods.iter().filter(|m| !m.on && m.role != BisectRole::KeptOff)
    }

    #[must_use]
    pub fn role_of(&self, hash: &str) -> Option<BisectRole> {
        self.mods.iter().find(|m| m.hash == hash).map(|m| m.role)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BisectAnswer {
    Broken,
    Works,
    Unsure,
}

#[must_use]
pub fn estimate_launches(mods: usize) -> u32 {
    if mods == 0 {
        0
    } else {
        usize::BITS - (mods - 1).leading_zeros() + 1
    }
}

static OPS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

static MOD_META: std::sync::LazyLock<std::sync::Mutex<HashMap<String, ModMeta>>> =
    std::sync::LazyLock::new(Default::default);

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
struct Flags {
    required: bool,
    kept_off: bool,
    omitted: bool,
    missing: bool,
    problem: bool,
    unresolvable: bool,
    was_enabled: bool,
}

impl Flags {
    fn activatable(self) -> bool {
        !(self.kept_off || self.missing || self.unresolvable || self.problem)
    }

    fn candidate(self) -> bool {
        self.activatable() && !self.required && !self.omitted
    }

    fn evaluated(self) -> bool {
        !(self.kept_off || self.missing || self.problem)
    }

    fn choice(self) -> BisectChoice {
        if self.required {
            BisectChoice::Required
        } else if self.kept_off {
            BisectChoice::KeptOff
        } else if self.omitted {
            BisectChoice::Omitted
        } else {
            BisectChoice::Search
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    state: State,
    #[serde(default)]
    log: Vec<Test>,
    #[serde(default)]
    found: Vec<Set>,
    #[serde(default)]
    pending: Set,
    #[serde(default)]
    flags: BTreeMap<String, Flags>,
    #[serde(default)]
    ignored: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    hidden_checked: bool,
    #[serde(default)]
    hidden: Vec<(String, String, usize)>,
    #[serde(default)]
    problem: Option<String>,
}

impl Saved {
    fn fresh(hashes: &[String]) -> Self {
        Self {
            state: State::new(hashes.iter().cloned().collect(), 1),
            log: Vec::new(),
            found: Vec::new(),
            pending: Set::new(),
            flags: hashes
                .iter()
                .map(|hash| {
                    (
                        hash.clone(),
                        Flags {
                            was_enabled: true,
                            ..Flags::default()
                        },
                    )
                })
                .collect(),
            ignored: BTreeMap::new(),
            hidden_checked: false,
            hidden: Vec::new(),
            problem: None,
        }
    }

    fn select(&self, keep: impl Fn(Flags) -> bool) -> Set {
        self.flags
            .iter()
            .filter(|(_, f)| keep(**f))
            .map(|(hash, _)| hash.clone())
            .collect()
    }

    fn activatable(&self) -> Set {
        self.select(Flags::activatable)
    }

    fn forced(&self) -> Set {
        self.select(|f| f.required && f.activatable())
    }

    fn candidates(&self) -> Set {
        self.select(Flags::candidate)
    }

    fn present(&self) -> Set {
        self.select(|f| !f.missing)
    }
}

type Edge = (String, String, BisectEdgeKind);

struct Session {
    cluster_id: i64,
    pending_exit: BisectExit,
    retry_note: Option<String>,
    finishing: bool,
    hashes: Vec<String>,
    names: HashMap<String, String>,
    testing: Set,
    disable_on_finish: Set,
    linked: HashMap<String, LinkedArtifactInfo>,
    edges: Vec<Edge>,
    suggestions: Vec<(Option<String>, String)>,
    saved: Saved,
}

impl Session {
    fn name(&self, hash: &str) -> String {
        self.names
            .get(hash)
            .cloned()
            .unwrap_or_else(|| hash.to_string())
    }

    fn names(&self, hashes: &Set) -> String {
        self.name_list(hashes).join(", ")
    }

    fn name_list<'a>(&self, hashes: impl IntoIterator<Item = &'a String>) -> Vec<String> {
        let mut names: Vec<String> = hashes.into_iter().map(|h| self.name(h)).collect();
        names.sort_by_key(|n| n.to_lowercase());
        names
    }

    fn off(&self) -> Set {
        self.hashes
            .iter()
            .filter(|h| !self.testing.contains(*h) && self.linked.contains_key(*h))
            .cloned()
            .collect()
    }

    fn flags(&mut self, hash: &str) -> &mut Flags {
        self.saved.flags.entry(hash.to_string()).or_default()
    }
}

#[tracing::instrument(skip(state))]
pub async fn bisect_status(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<Option<BisectStatus>> {
    let guard = OPS.try_lock().ok();
    let Some(session) = load(state, cluster_id, guard.is_some()).await? else {
        return Ok(None);
    };
    let catalog = catalog(state, &session).await;
    Ok(Some(describe(&session, &catalog)))
}

#[tracing::instrument(skip(state, progress))]
pub async fn start_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    progress: impl Fn(&str, u64, u64),
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let db = &state.services.db;
    if bisect_dao::is_active(db, cluster_id).await? {
        return Err(BisectError::AlreadyRunning.into());
    }
    match bisect_dao::any_active(db).await? {
        Some(running) => {
            let name = match state.clusters.get(running).await {
                Ok(cluster) => cluster.name.clone(),
                Err(_) => "another cluster".to_string(),
            };
            return Err(BisectError::RunningElsewhere { name }.into());
        }
        None => {}
    }

    let linked = PackageStore::list_linked_artifacts(cluster_id, &state.services.content()).await?;
    let mut all: Vec<&LinkedArtifactInfo> = linked
        .iter()
        .filter(|info| info.content_type == ContentType::Mod)
        .collect();
    all.sort_by(|a, b| a.hash.cmp(&b.hash));
    all.dedup_by(|a, b| a.hash == b.hash);
    if all.iter().filter(|info| info.enabled).count() < 2 {
        return Err(BisectError::TooFewMods.into());
    }

    let total = all.len() as u64 + 2;
    for (read, info) in all.iter().enumerate() {
        progress("Reading mods", read as u64, total);
        mod_meta(state, &info.hash, &info.file_name).await;
    }

    progress("Checking dependencies online", total - 2, total);
    let names = mod_names(state, &all).await;
    let mods: Vec<(String, String)> = all
        .iter()
        .map(|info| {
            let name = names
                .get(&info.hash)
                .cloned()
                .unwrap_or_else(|| info.file_name.clone());
            (info.hash.clone(), name)
        })
        .collect();

    let hashes: Vec<String> = mods.iter().map(|(hash, _)| hash.clone()).collect();
    let mut saved = Saved::fresh(&hashes);
    for info in &all {
        if !info.enabled
            && let Some(flags) = saved.flags.get_mut(&info.hash)
        {
            flags.was_enabled = false;
            flags.kept_off = true;
        }
    }
    for loser in duplicate_losers(&all) {
        if let Some(flags) = saved.flags.get_mut(&loser) {
            flags.kept_off = true;
        }
    }

    bisect_dao::start(db, cluster_id, &mods, &encode(&saved)).await?;
    tracing::info!(cluster_id, mods = mods.len(), "started problem mod search");

    let enabled: Vec<&LinkedArtifactInfo> = all.iter().copied().filter(|i| i.enabled).collect();
    let listed = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        listed_dependency_edges(state, &enabled),
    )
    .await
    .unwrap_or_else(|_| {
        tracing::warn!(
            cluster_id,
            "reading the listed mod dependencies took too long"
        );
        Vec::new()
    });
    if !listed.is_empty()
        && let Err(err) = bisect_dao::add_edges(db, cluster_id, &listed).await
    {
        tracing::warn!(cluster_id, error = %err, "could not save the listed mod dependencies");
    }

    progress("Switching mods for the first round", total - 1, total);
    let started = match load(state, cluster_id, true).await {
        Ok(Some(session)) => fresh_round(state, session, 1).await,
        Ok(None) => Err(BisectError::NotRunning.into()),
        Err(err) => Err(err),
    };
    if started.is_err()
        && let Err(err) = bisect_dao::end(db, cluster_id).await
    {
        tracing::warn!(cluster_id, error = %err, "could not drop a mod search that failed to start");
    }
    started
}

fn duplicate_losers(all: &[&LinkedArtifactInfo]) -> Vec<String> {
    let cache = MOD_META.lock().ok();
    let mut by_id: HashMap<String, Vec<(&LinkedArtifactInfo, ModMeta)>> = HashMap::new();
    for info in all.iter().filter(|info| info.enabled) {
        if let Some(meta) = cache
            .as_ref()
            .and_then(|c| c.get(&info.hash))
            .filter(|meta| !meta.loose_id)
        {
            by_id
                .entry(meta.id.clone())
                .or_default()
                .push((info, meta.clone()));
        }
    }
    let mut losers = Vec::new();
    for mut group in by_id.into_values().filter(|g| g.len() > 1) {
        group.sort_by(|a, b| {
            b.1.version
                .compare(&a.1.version)
                .then_with(|| a.0.file_name.cmp(&b.0.file_name))
        });
        losers.extend(group.into_iter().skip(1).map(|(info, _)| info.hash.clone()));
    }
    losers
}

#[tracing::instrument(skip(state))]
pub async fn answer_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    answer: BisectAnswer,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;
    let plan = session
        .saved
        .state
        .plan()
        .ok_or(BisectError::NothingToAnswer)?;
    let outcome = match answer {
        BisectAnswer::Broken => Outcome::Fail,
        BisectAnswer::Works => Outcome::Good,
        BisectAnswer::Unsure => Outcome::Unsure,
    };

    let before = session.saved.state.clone();
    session.saved.state = before.apply(&plan, outcome);
    session.saved.log.push(Test {
        plan: plan.clone(),
        outcome,
        before,
        enabled: session.testing.clone(),
    });
    if plan.kind == BisectPlanKind::Verification && !session.saved.pending.is_empty() {
        let pending = std::mem::take(&mut session.saved.pending);
        session.saved.state.add(&pending);
    }
    tracing::info!(cluster_id, ?answer, kind = ?plan.kind, "recorded problem mod search answer");

    advance(state, session).await
}

#[tracing::instrument(skip(state))]
pub async fn skip_bisect_answer(state: &Arc<LauncherState>, cluster_id: i64) -> LauncherResult<()> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
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
    let _busy = ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;
    if !can_undo(&session.saved) {
        return Err(BisectError::NothingToUndo.into());
    }
    if let Some(last) = session.saved.log.pop() {
        session.saved.state = last.before;
    }
    advance(state, session).await
}

#[tracing::instrument(skip(state))]
pub async fn continue_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;
    let current = &session.saved.state;
    if !current.complete || current.conflict.is_empty() || current.candidate_set().is_empty() {
        return Err(BisectError::CannotContinue.into());
    }
    let conflict = current.conflict.clone();
    let round = current.round + 1;
    for hash in &conflict {
        session.flags(hash).problem = true;
    }
    session.saved.found.push(conflict);

    tracing::info!(cluster_id, round, "looking for another problem");
    fresh_round(state, session, round).await
}

#[tracing::instrument(skip(state))]
pub async fn reset_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let db = &state.services.db;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;
    bisect_dao::remove_edges(db, cluster_id, BisectEdgeKind::Inferred).await?;
    session
        .edges
        .retain(|(_, _, kind)| *kind != BisectEdgeKind::Inferred);
    for flags in session.saved.flags.values_mut() {
        flags.problem = false;
    }
    session.saved.found.clear();
    session.saved.log.clear();
    session.saved.hidden_checked = false;
    session.saved.hidden.clear();
    tracing::info!(cluster_id, "started the problem mod search over");
    fresh_round(state, session, 1).await
}

#[tracing::instrument(skip(state))]
pub async fn set_bisect_mod_choice(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    hashes: &[String],
    choice: BisectChoice,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;
    for hash in hashes {
        if !session.saved.flags.contains_key(hash) {
            continue;
        }
        let flags = session.flags(hash);
        if choice == BisectChoice::Required && flags.unresolvable {
            continue;
        }
        flags.required = choice == BisectChoice::Required;
        flags.kept_off = choice == BisectChoice::KeptOff;
        flags.omitted = choice == BisectChoice::Omitted;
    }
    advance(state, session).await
}

#[tracing::instrument(skip(state))]
pub async fn settle_unresolvable_mod(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    hash: &str,
    ignore: bool,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;
    if ignore {
        let catalog = catalog(state, &session).await;
        let evaluated = session.saved.select(Flags::evaluated);
        let failing = catalog
            .directly_unresolvable(&evaluated, false)
            .remove(hash)
            .unwrap_or_default();
        session
            .saved
            .ignored
            .entry(hash.to_string())
            .or_default()
            .extend(failing);
    } else {
        session.flags(hash).kept_off = true;
    }
    advance(state, session).await
}

#[tracing::instrument(skip(state, progress))]
pub async fn find_hidden_dependencies(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    progress: impl Fn(&str, u64, u64),
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;
    if !session.saved.state.halted {
        return Err(BisectError::NotHalted.into());
    }
    let catalog = catalog(state, &session).await;
    let activatable = session.saved.activatable();

    let total = activatable.len() as u64;
    let mut indexes: BTreeMap<String, ClassIndex> = BTreeMap::new();
    for (done, hash) in activatable.iter().enumerate() {
        progress("Reading mod classes", done as u64, total);
        if let Some(path) = artifact_path(state, hash).await {
            indexes.insert(hash.clone(), meta::index_classes(&path).await);
        }
    }

    let groups = halted_groups(&session.saved.state);
    let found: Vec<(String, String, usize)> = resolve::infer(&catalog, &indexes)
        .into_iter()
        .filter(|dep| activatable.contains(&dep.target))
        .filter(|dep| {
            groups.as_ref().is_none_or(|(a, b)| {
                (a.contains(&dep.source) && b.contains(&dep.target))
                    || (b.contains(&dep.source) && a.contains(&dep.target))
            })
        })
        .map(|dep| (dep.source, dep.target, dep.classes.len()))
        .collect();

    tracing::info!(cluster_id, found = found.len(), "looked for undeclared dependencies");
    session.saved.hidden_checked = found.is_empty();
    session.saved.hidden = found;
    bisect_dao::set_search(&state.services.db, cluster_id, &encode(&session.saved)).await?;
    if session.saved.hidden.is_empty() {
        let note = "No hidden dependencies were found between the two groups.";
        bisect_dao::set_retry_note(&state.services.db, cluster_id, note).await?;
        session.retry_note = Some(note.to_string());
    }
    state.services.events.signal(Signal::ClustersChanged);
    Ok(describe(&session, &catalog))
}

#[tracing::instrument(skip(state))]
pub async fn use_hidden_dependencies(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    accept: bool,
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;
    let hidden = std::mem::take(&mut session.saved.hidden);
    session.saved.hidden_checked = true;
    if accept && !hidden.is_empty() {
        let edges: Vec<Edge> = hidden
            .into_iter()
            .map(|(from, to, _)| (from, to, BisectEdgeKind::Inferred))
            .collect();
        learn(state, &mut session, edges).await?;
        session.saved.state.halted = false;
    }
    advance(state, session).await
}

#[tracing::instrument(skip(state))]
pub async fn finish_bisect(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    disable: &[String],
) -> LauncherResult<()> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let Some(session) = load(state, cluster_id, true).await? else {
        return Err(BisectError::NotRunning.into());
    };

    if !session.finishing {
        let disable: Vec<String> = disable
            .iter()
            .filter(|hash| session.hashes.contains(hash))
            .cloned()
            .collect();
        bisect_dao::begin_finish(&state.services.db, cluster_id, &disable).await?;
    }

    complete_finish(state, cluster_id).await
}

async fn complete_finish(state: &Arc<LauncherState>, cluster_id: i64) -> LauncherResult<()> {
    let ctx = state.services.content();
    let Some(session) = load(state, cluster_id, true).await? else {
        return Ok(());
    };

    let mut failure: Option<BisectError> = None;
    for hash in &session.hashes {
        let Some(info) = session.linked.get(hash) else {
            continue;
        };
        let disable = session.disable_on_finish.contains(hash);
        let was_enabled = session.saved.flags.get(hash).is_none_or(|f| f.was_enabled);
        let result = if disable {
            oneclient_content::bundles::set_artifact_enabled_to(cluster_id, hash, false, &ctx)
                .await
                .map(|_| ())
        } else if info.enabled != was_enabled {
            PackageStore::set_artifact_enabled_to(cluster_id, hash, was_enabled, &ctx)
                .await
                .map(|_| ())
        } else {
            Ok(())
        };
        if let Err(err) = result {
            tracing::warn!(cluster_id, %hash, error = %err, "could not restore a mod after the search");
            failure.get_or_insert(BisectError::SwitchFailed {
                name: session.name(hash),
                direction: if disable || !was_enabled { "off" } else { "back on" },
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
        disabled = session.disable_on_finish.len(),
        "finished problem mod search"
    );
    state.services.events.signal(Signal::ClustersChanged);
    Ok(())
}

pub async fn resume_unfinished_bisects(state: &Arc<LauncherState>) {
    let db = &state.services.db;
    match bisect_dao::list_finishing(db).await {
        Ok(clusters) => {
            for cluster_id in clusters {
                let _guard = OPS.lock().await;
                let Ok(_busy) = ensure_idle(state, cluster_id) else {
                    continue;
                };
                if let Err(err) = complete_finish(state, cluster_id).await {
                    tracing::warn!(cluster_id, error = %err, "could not finish restoring mods after a search");
                }
            }
        }
        Err(err) => tracing::warn!(error = %err, "could not look for unfinished mod searches"),
    }

    match bisect_dao::list_without_search(db).await {
        Ok(clusters) => {
            for cluster_id in clusters {
                let _guard = OPS.lock().await;
                let Ok(_busy) = ensure_idle(state, cluster_id) else {
                    continue;
                };
                let upgraded = match load(state, cluster_id, true).await {
                    Ok(Some(session)) => fresh_round(state, session, 1).await,
                    Ok(None) => Err(BisectError::NotRunning.into()),
                    Err(err) => Err(err),
                };
                match upgraded {
                    Ok(_) => tracing::info!(cluster_id, "moved a mod search to the new search"),
                    Err(err) => {
                        tracing::warn!(cluster_id, error = %err, "could not move a mod search to the new search");
                    }
                }
            }
        }
        Err(err) => tracing::warn!(error = %err, "could not look for old mod searches"),
    }
}

#[tracing::instrument(skip(state))]
pub async fn keep_bisect_mods_on(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    hashes: &[String],
) -> LauncherResult<BisectStatus> {
    let _guard = OPS.lock().await;
    let _busy = ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;

    let off = session.off();
    let targets: Set = hashes
        .iter()
        .filter(|hash| off.contains(*hash))
        .cloned()
        .collect();
    let edges = edges_from_testing(&session, &targets, BisectEdgeKind::Guessed);
    learn(state, &mut session, edges).await?;

    let kept = if targets.len() > 3 {
        format!("{} mods", targets.len())
    } else {
        session.names(&targets)
    };
    let note = format!("{kept} will stay on with the mods being tested. Launch again.");
    let mut status = advance(state, session).await?;
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
    let _busy = ensure_idle(state, cluster_id)?;
    let mut session = load(state, cluster_id, true)
        .await?
        .ok_or(BisectError::NotRunning)?;

    let off = session.off();
    let mut edges: Vec<Edge> = Vec::new();
    let mut targets = Set::new();
    let mut requesters = Set::new();
    for (from, to) in session.suggestions.clone() {
        if !off.contains(&to) {
            continue;
        }
        targets.insert(to.clone());
        match from.filter(|from| session.testing.contains(from)) {
            Some(from) => {
                requesters.insert(from.clone());
                edges.push((from, to, BisectEdgeKind::Known));
            }
            None => edges.extend(edges_from_testing(
                &session,
                &Set::from([to]),
                BisectEdgeKind::Guessed,
            )),
        }
    }

    if targets.is_empty() {
        bisect_dao::clear_suggestions(&state.services.db, cluster_id).await?;
        state.services.events.signal(Signal::ClustersChanged);
        return Err(BisectError::NoSuggestion.into());
    }

    learn(state, &mut session, edges).await?;

    let with = if requesters.is_empty() {
        "the mods being tested".to_string()
    } else {
        session.names(&requesters)
    };
    let note = format!("{} will stay on with {with}. Launch again.", session.names(&targets));
    let mut status = advance(state, session).await?;
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
    let _busy = ensure_idle(state, cluster_id)?;
    bisect_dao::clear_suggestions(&state.services.db, cluster_id).await?;
    state.services.events.signal(Signal::ClustersChanged);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GameExit {
    Clean,
    Crashed,
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
    let Some(mut session) = load(state, cluster_id, true).await? else {
        return Ok(false);
    };
    if session.saved.state.plan().is_none() {
        return Ok(false);
    }

    let catalog = catalog(state, &session).await;
    let off = session.off();
    let mut edges: Vec<Edge> = Vec::new();
    let mut requesters = Set::new();
    let mut unknown_requester = false;
    let mut targets = Set::new();

    for found in sure {
        let MissingTarget::Mod(id) = &found.missing else {
            continue;
        };
        let Some(target) = catalog.provider_of(&id.to_ascii_lowercase(), &off) else {
            continue;
        };
        targets.insert(target.clone());

        let requester = found
            .requester
            .as_ref()
            .and_then(|id| catalog.provider_of(&id.to_ascii_lowercase(), &session.testing));
        match requester {
            Some(from) => {
                requesters.insert(from.clone());
                edges.push((from, target, BisectEdgeKind::Known));
            }
            None => {
                unknown_requester = true;
                edges.extend(edges_from_testing(
                    &session,
                    &Set::from([target]),
                    BisectEdgeKind::Guessed,
                ));
            }
        }
    }

    if !learn(state, &mut session, edges).await? {
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

    advance(state, session).await?;
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
    let Some(session) = load(state, cluster_id, true).await? else {
        return Ok(false);
    };
    if session.saved.state.plan().is_none() {
        return Ok(false);
    }

    let off = session.off();
    let mut jars = JarEntries::default();
    let mut suggestions: Vec<(Option<String>, String)> = Vec::new();

    for found in classes {
        let MissingTarget::Class(class) = &found.missing else {
            continue;
        };
        let Some(target) = jars.find(state, &off, class).await else {
            continue;
        };

        let mut requester = None;
        for frame in &found.frames {
            if let Some(from) = jars.find(state, &session.testing, frame).await {
                requester = Some(from);
                break;
            }
        }

        let suggestion = (requester, target);
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

fn edges_from_testing(session: &Session, targets: &Set, kind: BisectEdgeKind) -> Vec<Edge> {
    session
        .testing
        .iter()
        .flat_map(|from| {
            targets
                .iter()
                .filter(move |to| *to != from)
                .map(move |to| (from.clone(), to.clone(), kind))
        })
        .collect()
}

async fn learn(
    state: &Arc<LauncherState>,
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

    bisect_dao::add_edges(&state.services.db, session.cluster_id, &fresh).await?;
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
    names: HashMap<String, HashSet<String>>,
}

impl JarEntries {
    async fn find(
        &mut self,
        state: &Arc<LauncherState>,
        candidates: &Set,
        class: &str,
    ) -> Option<String> {
        let entry = format!("{class}.class");
        for hash in candidates {
            if !self.names.contains_key(hash) {
                let names = match artifact_path(state, hash).await {
                    Some(path) => polyio::zip_entry_names(&path)
                        .await
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                    None => HashSet::new(),
                };
                self.names.insert(hash.clone(), names);
            }
            if self
                .names
                .get(hash)
                .is_some_and(|names| names.contains(&entry))
            {
                return Some(hash.clone());
            }
        }
        None
    }
}

async fn mod_meta(state: &Arc<LauncherState>, hash: &str, file_name: &str) -> Option<ModMeta> {
    if let Some(known) = MOD_META
        .lock()
        .ok()
        .and_then(|cache| cache.get(hash).cloned())
    {
        return Some(known);
    }
    let path = artifact_path(state, hash).await?;
    let meta = meta::read_mod(&path, file_name).await?;
    if let Ok(mut cache) = MOD_META.lock() {
        cache.insert(hash.to_string(), meta.clone());
    }
    Some(meta)
}

async fn artifact_path(state: &Arc<LauncherState>, hash: &str) -> Option<PathBuf> {
    match artifact_dao::get_artifact_by_hash(&state.services.db, hash).await {
        Ok(Some(row)) => artifact_absolute_path(&row.path).ok(),
        _ => None,
    }
}

static SWITCHING: std::sync::Mutex<BTreeSet<i64>> = std::sync::Mutex::new(BTreeSet::new());

/// Marks a cluster's mods as being switched until dropped, so a launch refuses to start meanwhile
struct Switching(i64);

impl Drop for Switching {
    fn drop(&mut self) {
        SWITCHING.lock().unwrap().remove(&self.0);
    }
}

/// Whether a search is changing this cluster's mods right now
#[must_use]
pub fn is_switching(cluster_id: i64) -> bool {
    SWITCHING.lock().unwrap().contains(&cluster_id)
}

/// Claims the cluster before checking for a game, while a launch marks itself active before
/// checking `is_switching`, so the two can't both go ahead
fn ensure_idle(state: &Arc<LauncherState>, cluster_id: i64) -> Result<Switching, BisectError> {
    SWITCHING.lock().unwrap().insert(cluster_id);
    let claim = Switching(cluster_id);
    if state.games.is_active(cluster_id) {
        Err(BisectError::GameRunning)
    } else {
        Ok(claim)
    }
}

fn encode(saved: &Saved) -> String {
    serde_json::to_string(saved).unwrap_or_default()
}

async fn load(
    state: &Arc<LauncherState>,
    cluster_id: i64,
    join: bool,
) -> LauncherResult<Option<Session>> {
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

    let mut hashes: Vec<String> = mods.iter().map(|m| m.hash.clone()).collect();
    let mut testing: Set = mods
        .iter()
        .filter(|m| m.testing != 0)
        .map(|m| m.hash.clone())
        .collect();
    let mut saved = row
        .search
        .as_deref()
        .and_then(|json| match serde_json::from_str::<Saved>(json) {
            Ok(saved) => Some(saved),
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "could not read the saved mod search");
                None
            }
        })
        .unwrap_or_else(|| Saved::fresh(&hashes));
    for hash in &hashes {
        saved
            .flags
            .entry(hash.clone())
            .or_insert(Flags {
                was_enabled: true,
                ..Flags::default()
            })
            .missing = !linked.contains_key(hash);
    }

    let joined: Vec<&LinkedArtifactInfo> = linked
        .values()
        .filter(|info| info.enabled && !hashes.contains(&info.hash))
        .collect();
    if join && row.search.is_some() && row.finishing == 0 && !joined.is_empty() {
        let found: Vec<(String, String)> = mod_names(state, &joined).await.into_iter().collect();
        for (hash, _) in &found {
            saved.flags.insert(
                hash.clone(),
                Flags {
                    required: true,
                    was_enabled: true,
                    ..Flags::default()
                },
            );
        }
        match bisect_dao::add_mods(db, cluster_id, &found, &encode(&saved)).await {
            Ok(()) => {
                for (hash, _) in &found {
                    hashes.push(hash.clone());
                    testing.insert(hash.clone());
                }
                names.extend(found);
            }
            Err(err) => {
                tracing::warn!(cluster_id, error = %err, "could not add mods that joined during the search");
                for (hash, _) in &found {
                    saved.flags.remove(hash);
                }
            }
        }
    }

    Ok(Some(Session {
        cluster_id,
        pending_exit: row.pending_exit(),
        retry_note: row.retry_note,
        finishing: row.finishing != 0,
        testing,
        disable_on_finish: mods
            .iter()
            .filter(|m| m.disable_on_finish != 0)
            .map(|m| m.hash.clone())
            .collect(),
        hashes,
        names,
        linked,
        edges,
        suggestions,
        saved,
    }))
}

async fn mod_names(
    state: &Arc<LauncherState>,
    infos: &[&LinkedArtifactInfo],
) -> HashMap<String, String> {
    let ctx = state.services.content();
    let key = |info: &LinkedArtifactInfo| match (info.provider, &info.project_id) {
        (Some(provider), Some(project_id)) if provider != ProviderId::Local => {
            (provider, project_id.clone())
        }
        _ => (ProviderId::Local, info.hash.clone()),
    };
    let mut by_provider: HashMap<ProviderId, Vec<String>> = HashMap::new();
    for info in infos {
        let (provider, id) = key(info);
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
        let name = match project_names.get(&key(info)).cloned() {
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

async fn overrides(state: &Arc<LauncherState>, cluster_id: i64) -> Vec<resolve::Rule> {
    let mut layers = Vec::new();
    if let Ok(cluster) = state.clusters.get(cluster_id).await
        && let Ok(dir) = cluster.game_dir()
    {
        let path = dir.join("config").join("dependency_overrides.json");
        if let Ok(text) = tokio::fs::read_to_string(&path).await {
            match resolve::parse_overrides(&text) {
                Ok(rules) => layers.push(rules),
                Err(err) => {
                    tracing::warn!(cluster_id, path = %path.display(), %err, "could not read dependency overrides");
                }
            }
        }
    }
    if let Ok(builtin) = resolve::parse_overrides(resolve::BUILTIN_OVERRIDES) {
        layers.push(builtin);
    }
    resolve::merge_overrides(layers)
}

async fn catalog(state: &Arc<LauncherState>, session: &Session) -> Catalog {
    let rules = overrides(state, session.cluster_id).await;
    let mut mods = BTreeMap::new();
    for hash in &session.hashes {
        let Some(info) = session.linked.get(hash) else {
            continue;
        };
        let mut meta = mod_meta(state, hash, &info.file_name)
            .await
            .unwrap_or_else(|| ModMeta::library(&info.file_name));
        resolve::apply_overrides(&rules, &mut meta);
        mods.insert(hash.clone(), meta);
    }
    let mut extra: BTreeMap<String, Set> = BTreeMap::new();
    for (from, to, kind) in &session.edges {
        let Some(id) = mods.get(to).map(|m| m.id.clone()) else {
            continue;
        };
        if from == to || !mods.contains_key(from) {
            continue;
        }
        if *kind == BisectEdgeKind::Guessed {
            extra.entry(from.clone()).or_default().insert(to.clone());
        } else if let Some(meta) = mods.get_mut(from) {
            meta.depends.entry(id).or_insert_with(|| vec![Predicate::any()]);
        }
    }
    for (hash, deps) in &session.saved.ignored {
        if let Some(meta) = mods.get_mut(hash) {
            for dep in deps {
                meta.forget(dep);
            }
        }
    }
    Catalog::new(mods, extra)
}

fn reconcile(saved: &mut Saved, catalog: &Catalog) {
    let evaluated = saved.select(Flags::evaluated);
    let unresolvable = catalog.unresolvable_details(&evaluated).all();
    for (hash, flags) in &mut saved.flags {
        flags.unresolvable = unresolvable.contains(hash);
    }

    let valid = saved.candidates();
    let current: Set = saved
        .state
        .candidates
        .iter()
        .chain(&saved.state.conflict)
        .chain(&saved.pending)
        .cloned()
        .collect();
    let removals: Set = current.difference(&valid).cloned().collect();
    if !removals.is_empty() {
        saved.pending.retain(|h| !removals.contains(h));
        saved.state.remove(&removals);
    }
    saved.pending.extend(valid.difference(&current).cloned());

    let search = &saved.state;
    if search.complete && search.conflict.is_empty() && !saved.pending.is_empty() {
        let round = search.round;
        saved.state = State::new(saved.candidates(), round);
        saved.pending.clear();
    }
}

fn can_undo(saved: &Saved) -> bool {
    saved
        .log
        .last()
        .is_some_and(|test| test.before.round == saved.state.round)
}

async fn fresh_round(
    state: &Arc<LauncherState>,
    mut session: Session,
    round: u32,
) -> LauncherResult<BisectStatus> {
    let catalog = catalog(state, &session).await;
    reconcile(&mut session.saved, &catalog);
    session.saved.state = State::new(session.saved.candidates(), round);
    session.saved.pending.clear();
    apply(state, session, &catalog).await
}

async fn advance(state: &Arc<LauncherState>, mut session: Session) -> LauncherResult<BisectStatus> {
    let catalog = catalog(state, &session).await;
    reconcile(&mut session.saved, &catalog);
    apply(state, session, &catalog).await
}

async fn apply(
    state: &Arc<LauncherState>,
    mut session: Session,
    catalog: &Catalog,
) -> LauncherResult<BisectStatus> {
    let cluster_id = session.cluster_id;
    let activatable = session.saved.activatable();
    let forced = session.saved.forced();
    let want: Set = match session.saved.state.plan() {
        Some(plan) => {
            let resolution = catalog.resolve(&plan.targets(), &forced, &activatable);
            if let Some(problem) = &resolution.problem {
                tracing::warn!(cluster_id, %problem, "the mods for this round don't all fit together");
            }
            session.saved.problem = resolution.problem;
            resolution
                .set
                .union(&forced)
                .filter(|h| activatable.contains(*h))
                .cloned()
                .collect()
        }
        None => {
            session.saved.problem = None;
            session.testing.clone()
        }
    };

    let switches: Vec<(String, bool)> = session
        .hashes
        .iter()
        .filter_map(|hash| {
            let info = session.linked.get(hash)?;
            let on = want.contains(hash);
            (info.enabled != on).then(|| (hash.clone(), on))
        })
        .collect();

    let ctx = state.services.content();
    let mut done: Vec<(String, bool)> = Vec::new();
    for (hash, on) in &switches {
        if let Err(err) = PackageStore::set_artifact_enabled_to(cluster_id, hash, *on, &ctx).await {
            tracing::warn!(cluster_id, %hash, error = %err, "could not switch a mod for the search");
            undo_switches(state, cluster_id, &done).await;
            return Err(BisectError::SwitchFailed {
                name: session.name(hash),
                direction: if *on { "on" } else { "off" },
                reason: err.to_string(),
            }
            .into());
        }
        done.push((hash.clone(), *on));
    }
    for (hash, on) in &done {
        if let Some(info) = session.linked.get_mut(hash) {
            info.enabled = *on;
        }
    }

    session.testing = want;
    let states: Vec<BisectModState<'_>> = session
        .hashes
        .iter()
        .map(|hash| BisectModState {
            hash,
            testing: session.testing.contains(hash),
        })
        .collect();
    if let Err(err) =
        bisect_dao::save(&state.services.db, cluster_id, &encode(&session.saved), &states).await
    {
        // The saved round still names the old mods, so the files go back to match it
        undo_switches(state, cluster_id, &done).await;
        return Err(err.into());
    }
    session.pending_exit = BisectExit::None;
    session.retry_note = None;
    session.suggestions.clear();

    state.services.events.signal(Signal::ClustersChanged);
    Ok(describe(&session, catalog))
}

async fn undo_switches(state: &Arc<LauncherState>, cluster_id: i64, done: &[(String, bool)]) {
    let ctx = state.services.content();
    for (hash, on) in done.iter().rev() {
        if let Err(err) = PackageStore::set_artifact_enabled_to(cluster_id, hash, !on, &ctx).await {
            tracing::warn!(cluster_id, %hash, error = %err, "could not undo a switch for the search");
        }
    }
    state.services.events.signal(Signal::ClustersChanged);
}

fn halted_groups(state: &State) -> Option<(Set, Set)> {
    if !state.halted {
        return None;
    }
    let candidates: Vec<String> = state.candidate_set().into_iter().collect();
    let (first, second) = imcs::split(&candidates);
    Some((first.into_iter().collect(), second.into_iter().collect()))
}

fn conflict_report(session: &Session, catalog: &Catalog, conflict: &Set) -> ConflictReport {
    let all = session.saved.present();
    let generally = catalog.transitively_unresolvable(&all, false);
    let with_extra = catalog.has_extra();
    let without = |gone: &Set| -> Set { all.difference(gone).cloned().collect() };

    let mut union = Set::new();
    let mut mods = Vec::new();
    for hash in conflict {
        let gone = Set::from([hash.clone()]);
        let cascade = catalog.transitively_unresolvable(&without(&gone), false);
        let also: Set = cascade.difference(&generally).cloned().collect();
        union.extend(also.iter().cloned());
        let maybe: Set = if with_extra {
            catalog
                .transitively_unresolvable(&without(&gone), true)
                .difference(&cascade)
                .filter(|h| *h != hash)
                .cloned()
                .collect()
        } else {
            Set::new()
        };
        mods.push(CascadeMod {
            hash: hash.clone(),
            name: session.name(hash),
            also: session.name_list(&also),
            maybe: session.name_list(&maybe),
        });
    }
    mods.sort_by_key(|m| m.name.to_lowercase());

    let full = catalog.transitively_unresolvable(&without(conflict), false);
    let if_all_also: Set = full
        .difference(&generally)
        .filter(|h| !union.contains(*h))
        .cloned()
        .collect();
    let if_all_maybe: Set = if with_extra {
        catalog
            .transitively_unresolvable(&without(conflict), true)
            .difference(&full)
            .filter(|h| !conflict.contains(*h))
            .cloned()
            .collect()
    } else {
        Set::new()
    };
    ConflictReport {
        mods,
        if_all_also: session.name_list(&if_all_also),
        if_all_maybe: session.name_list(&if_all_maybe),
    }
}

fn describe(session: &Session, catalog: &Catalog) -> BisectStatus {
    let saved = &session.saved;
    let search = &saved.state;
    let plan: Option<Plan> = search.plan();
    let tested: Set = plan
        .as_ref()
        .map(|p| p.tested_half().iter().cloned().collect())
        .unwrap_or_default();
    let stable: Set = plan.as_ref().map(|p| p.stable.clone()).unwrap_or_default();

    let mut mods: Vec<BisectMod> = session
        .hashes
        .iter()
        .filter_map(|hash| {
            session.linked.get(hash)?;
            let flags = saved.flags.get(hash).copied().unwrap_or_default();
            let on = session.testing.contains(hash);
            let role = if flags.problem {
                BisectRole::Problem
            } else if search.conflict.contains(hash) {
                BisectRole::Found
            } else if flags.unresolvable {
                BisectRole::Unresolvable
            } else if flags.kept_off {
                BisectRole::KeptOff
            } else if flags.required {
                BisectRole::Required
            } else if on && tested.contains(hash) {
                BisectRole::Testing
            } else if on && stable.contains(hash) {
                BisectRole::Cleared
            } else if on {
                BisectRole::Helper
            } else if flags.omitted {
                BisectRole::Omitted
            } else {
                BisectRole::Waiting
            };
            Some(BisectMod {
                hash: hash.clone(),
                name: session.name(hash),
                role,
                choice: flags.choice(),
                on,
            })
        })
        .collect();
    mods.sort_by_key(|m| m.name.to_lowercase());

    let stage = if search.halted {
        BisectStage::Halted
    } else if search.complete || plan.is_none() {
        BisectStage::Complete
    } else if plan.as_ref().is_some_and(|p| p.kind == BisectPlanKind::Verification) {
        BisectStage::Verifying
    } else {
        BisectStage::Testing
    };

    let evaluated = saved.select(Flags::evaluated);
    let details = catalog.unresolvable_details(&evaluated);
    let mut blocked_by: BTreeMap<&String, Set> = BTreeMap::new();
    for (hash, roots) in &details.transitive {
        for root in roots {
            blocked_by.entry(root).or_default().insert(hash.clone());
        }
    }
    let mut unresolvable: Vec<UnresolvableMod> = details
        .direct
        .iter()
        .map(|(hash, missing)| UnresolvableMod {
            hash: hash.clone(),
            name: session.name(hash),
            missing: missing.clone(),
            blocks: session.name_list(blocked_by.get(hash).into_iter().flatten()),
        })
        .collect();
    unresolvable.sort_by_key(|m| m.name.to_lowercase());

    let history = saved
        .log
        .iter()
        .map(|test| {
            let tested: Vec<String> = match test.plan.kind {
                BisectPlanKind::Verification => session.name_list(&test.plan.conflict),
                _ => session.name_list(test.plan.tested_half()),
            };
            BisectHistoryEntry {
                round: test.before.round,
                iteration: test.before.iteration,
                step: test.before.step,
                kind: test.plan.kind,
                answer: match test.outcome {
                    Outcome::Fail => BisectAnswer::Broken,
                    Outcome::Good => BisectAnswer::Works,
                    Outcome::Unsure => BisectAnswer::Unsure,
                },
                tested,
                enabled: test.enabled.len(),
            }
        })
        .collect();

    let suggestion = (!session.suggestions.is_empty()).then(|| {
        let mut missing: Vec<String> = Vec::new();
        let mut needed_by: Vec<String> = Vec::new();
        for (from, to) in &session.suggestions {
            let name = session.name(to);
            if !missing.contains(&name) {
                missing.push(name);
            }
            if let Some(from) = from {
                let name = session.name(from);
                if !needed_by.contains(&name) {
                    needed_by.push(name);
                }
            }
        }
        BisectSuggestion { missing, needed_by }
    });

    let groups = halted_groups(search).map(|(a, b)| (session.name_list(&a), session.name_list(&b)));

    BisectStatus {
        cluster_id: session.cluster_id,
        stage,
        round: search.round,
        iteration: search.iteration,
        tests_this_round: saved
            .log
            .iter()
            .filter(|test| test.before.round == search.round)
            .count(),
        estimate: search.estimate(),
        pending_exit: session.pending_exit,
        mods,
        current: conflict_report(session, catalog, &search.conflict),
        earlier: saved
            .found
            .iter()
            .map(|set| conflict_report(session, catalog, set))
            .collect(),
        pending: session.name_list(&saved.pending),
        removed: session.name_list(saved.flags.iter().filter(|(_, f)| f.missing).map(|(h, _)| h)),
        can_undo: can_undo(saved),
        can_continue: search.complete
            && !search.conflict.is_empty()
            && !search.candidate_set().is_empty(),
        groups,
        can_look_for_hidden: search.halted
            && search.candidate_set().len() > 1
            && !saved.hidden_checked
            && saved.hidden.is_empty(),
        hidden: saved
            .hidden
            .iter()
            .map(|(from, to, classes)| HiddenDependency {
                from: session.name(from),
                to: session.name(to),
                classes: *classes,
            })
            .collect(),
        unresolvable,
        history,
        resolution_problem: saved.problem.clone(),
        retry_note: session.retry_note.clone(),
        suggestion,
        finishing: session.finishing,
    }
}

#[tracing::instrument(skip(state))]
pub async fn bisect_report(state: &Arc<LauncherState>, cluster_id: i64) -> LauncherResult<String> {
    let guard = OPS.try_lock().ok();
    let session = load(state, cluster_id, guard.is_some())
        .await?
        .ok_or(BisectError::NotRunning)?;
    let catalog = catalog(state, &session).await;
    let status = describe(&session, &catalog);
    Ok(report(&session, &status))
}

fn report(session: &Session, status: &BisectStatus) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let stage = match status.stage {
        BisectStage::Testing => "In progress",
        BisectStage::Verifying => "Checking the found mods alone",
        BisectStage::Halted => "Stuck: both halves were unclear",
        BisectStage::Complete => "Complete",
    };
    let _ = writeln!(out, "OneClient problem mod search report");
    let _ = writeln!(out, "===================================");
    let _ = writeln!(out, "Status: {stage}");
    let _ = writeln!(
        out,
        "Round {}, iteration {}, {} tests this round (about {} expected)",
        status.round, status.iteration, status.tests_this_round, status.estimate
    );
    if let Some(problem) = &status.resolution_problem {
        let _ = writeln!(out, "Dependency problem this round: {problem}");
    }

    let write_set = |out: &mut String, title: &str, report: &ConflictReport| {
        let _ = writeln!(out, "\n{title}");
        if report.mods.is_empty() {
            let _ = writeln!(out, "  (none)");
        }
        for m in &report.mods {
            let _ = writeln!(out, "  - {}", m.name);
            if !m.also.is_empty() {
                let _ = writeln!(out, "      turning it off also turns off: {}", m.also.join(", "));
            }
            if !m.maybe.is_empty() {
                let _ = writeln!(out, "      may also need off: {}", m.maybe.join(", "));
            }
        }
        if !report.if_all_also.is_empty() {
            let _ = writeln!(out, "  turning all of them off also turns off: {}", report.if_all_also.join(", "));
        }
    };
    for (index, earlier) in status.earlier.iter().enumerate() {
        write_set(&mut out, &format!("Problem #{}", index + 1), earlier);
    }
    write_set(&mut out, "Current problem", &status.current);

    if !status.unresolvable.is_empty() {
        let _ = writeln!(out, "\nMods that can't load");
        for m in &status.unresolvable {
            let _ = writeln!(out, "  - {} is missing {}", m.name, m.missing.join(", "));
            if !m.blocks.is_empty() {
                let _ = writeln!(out, "      which also blocks: {}", m.blocks.join(", "));
            }
        }
    }

    let _ = writeln!(out, "\nTests");
    for (index, test) in session.saved.log.iter().enumerate() {
        let answer = match test.outcome {
            Outcome::Fail => "BROKEN",
            Outcome::Good => "WORKS",
            Outcome::Unsure => "UNCLEAR",
        };
        let kind = match test.plan.kind {
            BisectPlanKind::Verification => " [VERIFICATION]",
            BisectPlanKind::Complement => " [OTHER HALF]",
            _ => "",
        };
        let _ = writeln!(
            out,
            "  #{} round {} iteration {} step {}{kind}: {answer}",
            index + 1,
            test.before.round,
            test.before.iteration,
            test.before.step
        );
        let _ = writeln!(out, "      tested: {}", session.name_list(&test.plan.targets()).join(", "));
        let _ = writeln!(out, "      found so far: {}", session.name_list(&test.before.conflict).join(", "));
        let _ = writeln!(out, "      suspects: {}", session.name_list(&test.before.candidate_set()).join(", "));
        let _ = writeln!(out, "      cleared: {}", session.name_list(&test.before.cleared_set()).join(", "));
        let _ = writeln!(out, "      mods on: {}", test.enabled.len());
    }

    let search = &session.saved.state;
    let _ = writeln!(out, "\nRight now");
    let _ = writeln!(out, "  suspects: {}", session.name_list(&search.candidate_set()).join(", "));
    let _ = writeln!(out, "  cleared: {}", session.name_list(&search.cleared_set()).join(", "));
    if !status.pending.is_empty() {
        let _ = writeln!(out, "  joining after the next check: {}", status.pending.join(", "));
    }
    out
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

    #[test]
    fn estimate_matches_the_search() {
        for mods in 0..300 {
            let ids: Set = (0..mods).map(|i| i.to_string()).collect();
            assert_eq!(estimate_launches(mods), State::new(ids, 1).estimate(), "{mods}");
        }
    }

    fn hashes(items: &[&str]) -> Set {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn reconcile_keeps_found_mods_out_and_queues_returning_ones() {
        let all = ["a", "b", "c", "d"].map(String::from);
        let mut saved = Saved::fresh(&all);
        saved.state.conflict = hashes(&["a"]);
        saved.state.candidates = vec!["b".to_string(), "c".to_string()];
        let catalog = Catalog::new(BTreeMap::new(), BTreeMap::new());

        reconcile(&mut saved, &catalog);
        assert_eq!(saved.pending, hashes(&["d"]));
        assert_eq!(saved.state.conflict, hashes(&["a"]));

        saved.flags.get_mut("c").unwrap().kept_off = true;
        saved.flags.get_mut("d").unwrap().omitted = true;
        reconcile(&mut saved, &catalog);
        assert_eq!(saved.state.candidates, vec!["b".to_string()]);
        assert!(saved.pending.is_empty());
    }

    #[test]
    fn mods_waiting_to_join_are_searched_when_nothing_was_found() {
        let all = ["a", "b", "c"].map(String::from);
        let mut saved = Saved::fresh(&all);
        saved.state = State::new(hashes(&["a", "b"]), 1);
        saved.state.complete = true;
        let catalog = Catalog::new(BTreeMap::new(), BTreeMap::new());

        reconcile(&mut saved, &catalog);
        assert!(saved.pending.is_empty());
        assert!(!saved.state.complete);
        assert_eq!(saved.state.candidate_set(), hashes(&["a", "b", "c"]));
    }

    #[test]
    fn always_on_survives_a_dependency_being_kept_off_for_a_while() {
        let all = ["x", "y", "z"].map(String::from);
        let mut saved = Saved::fresh(&all);
        let mut needs_y = ModMeta::library("x.jar");
        needs_y.depends.insert(
            ModMeta::library("y.jar").id,
            vec![Predicate::any()],
        );
        let catalog = Catalog::new(
            BTreeMap::from([
                ("x".to_string(), needs_y),
                ("y".to_string(), ModMeta::library("y.jar")),
                ("z".to_string(), ModMeta::library("z.jar")),
            ]),
            BTreeMap::new(),
        );
        saved.flags.get_mut("x").unwrap().required = true;
        saved.flags.get_mut("y").unwrap().kept_off = true;
        reconcile(&mut saved, &catalog);
        assert!(saved.flags["x"].unresolvable);

        saved.flags.get_mut("y").unwrap().kept_off = false;
        reconcile(&mut saved, &catalog);
        assert_eq!(saved.flags["x"].choice(), BisectChoice::Required);
        assert!(!saved.candidates().contains("x"));
    }

    #[test]
    fn undo_stops_at_the_start_of_a_round() {
        let mut saved = Saved::fresh(&["a".to_string(), "b".to_string()]);
        assert!(!can_undo(&saved));
        let plan = saved.state.plan().unwrap();
        saved.log.push(Test {
            plan: plan.clone(),
            outcome: Outcome::Good,
            before: saved.state.clone(),
            enabled: Set::new(),
        });
        saved.state = saved.state.apply(&plan, Outcome::Good);
        assert!(can_undo(&saved));
        saved.state = State::new(hashes(&["b"]), 2);
        assert!(!can_undo(&saved));
    }
}
