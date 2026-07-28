use std::collections::HashMap;
use std::time::{Duration, Instant};

use oneclient_events::{
    Answer, Choice, Event, GroupedProgressEvent, Level, Notification, ProgressEvent, TaskCategory,
};
use oneclient_content::packages::ProviderId;
use oneclient_db::models::ClusterId;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::components::IconType;
use crate::transfer::{TransferMeter, TransferStats};

pub const MESSAGE_TOAST_TTL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq)]
pub struct NotificationAction {
    pub label: String,
    pub kind: NotificationActionKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NotificationActionKind {
    /// Opens the changes modal for every cluster touched by one sync. Kept as a
    /// list so a batch sync stays a single "View changes" action instead of one
    /// button per cluster.
    OpenClusterUpdate(Vec<ClusterUpdateSummary>),
}

/// One changed package in a cluster update, carrying enough identity to
/// resolve a pretty display name through the package-meta cache in the UI.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterUpdateItem {
    pub provider: ProviderId,
    pub project_id: Option<String>,
    /// Name shown when the meta cache has no entry (file name / package id).
    pub fallback: String,
}

impl ClusterUpdateItem {
    pub fn from_name(name: impl Into<String>) -> Self {
        Self {
            provider: ProviderId::Local,
            project_id: None,
            fallback: name.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClusterUpdateSummary {
    pub cluster_id: ClusterId,
    pub cluster_name: String,
    pub updated: Vec<ClusterUpdateItem>,
    pub added: Vec<ClusterUpdateItem>,
    pub removed: Vec<ClusterUpdateItem>,
}

impl ClusterUpdateSummary {
    pub fn total(&self) -> usize {
        self.updated.len() + self.added.len() + self.removed.len()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InboxEntry {
    pub level: Level,
    pub id: u64,
    pub title: String,
    pub body: String,
    pub icon: Option<IconType>,
    pub progress: Option<(u64, u64)>,
    pub is_loading: bool,
    pub read: bool,
    pub created_at: Instant,
    pub actions: Vec<NotificationAction>,
    pub tasks: Vec<TaskView>,
    /// Live transfer stats for grouped downloads (bytes/sec, seconds remaining).
    pub transfer: Option<TransferStats>,
}

/// One row in the expandable task list, an aggregate over all children of a
/// single [`TaskCategory`] (e.g. all libraries collapse into one "Libraries" row).
#[derive(Clone, Debug, PartialEq)]
pub struct TaskView {
    pub label: String,
    pub phase: &'static str,
    pub current: u64,
    pub total: u64,
    pub done_count: u64,
    pub total_count: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NotificationSpec {
    pub title: String,
    pub body: String,
    pub level: Level,
    pub icon: Option<IconType>,
    pub progress: Option<(u64, u64)>,
    pub actions: Vec<NotificationAction>,
}

impl InboxEntry {
    pub fn dismissable(&self) -> bool {
        !self.is_loading
    }

    pub fn click_dismissable(&self) -> bool {
        self.dismissable() && self.actions.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct ActiveToast {
    #[allow(dead_code)]
    pub toast_id: u64,
    pub entry_id: u64,
}

#[derive(Debug)]
pub struct PendingPrompt {
    pub title: String,
    pub question: String,
    pub choices: Vec<Choice>,
    pub dismiss: Option<String>,
    /// Taken when answered, so a second answer cannot fire the oneshot twice.
    pub reply_tx: Option<oneshot::Sender<Option<Answer>>>,
}

/// The renderable half of a [`PendingPrompt`]: everything except the reply
/// channel, which cannot be cloned into the snapshot.
#[derive(Clone, Debug)]
pub struct PendingPromptView {
    pub title: String,
    pub question: String,
    pub choices: Vec<Choice>,
    pub dismiss: Option<String>,
}

impl PendingPromptView {
    /// Whether this prompt offers a choice with the given id. Lets a
    /// specialised overlay (the Java one) recognise a prompt it knows how to
    /// render richly, without the event layer naming the subsystem.
    pub fn has_choice(&self, id: &str) -> bool {
        self.choices.iter().any(|choice| choice.id == id)
    }

    pub fn choice(&self, id: &str) -> Option<&Choice> {
        self.choices.iter().find(|choice| choice.id == id)
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ToastDismissTimer {
    pub toast_id: u64,
    pub entry_id: u64,
    pub after: Duration,
}

#[derive(Clone, Debug, Default)]
pub struct NotificationSnapshot {
    pub inbox: Vec<InboxEntry>,
    pub center_open: bool,
    pub pending_prompt: Option<PendingPromptView>,
    pub cluster_update: Option<Vec<ClusterUpdateSummary>>,
    pub active_toast_entry_ids: Vec<u64>,
}

impl NotificationSnapshot {
    pub fn unread_count(&self) -> usize {
        NotificationState::unread_count(&self.inbox)
    }
}

#[derive(Default)]
pub struct NotificationState {
    next_id: u64,
    active_toasts: Vec<ActiveToast>,
    progress_entries: HashMap<Uuid, u64>,
    grouped_entries: HashMap<Uuid, u64>,
    grouped_tasks: HashMap<Uuid, GroupedTasks>,
    pending_timers: Vec<ToastDismissTimer>,
    cluster_update: Option<Vec<ClusterUpdateSummary>>,
}

/// Fixed display order for the category rows.
const CATEGORY_ORDER: [TaskCategory; 7] = [
    TaskCategory::Java,
    TaskCategory::Client,
    TaskCategory::Libraries,
    TaskCategory::Natives,
    TaskCategory::Assets,
    TaskCategory::Metadata,
    TaskCategory::Packages,
];

struct ChildRec {
    category: TaskCategory,
    phase: &'static str,
    current: u64,
    total: u64,
}

#[derive(Default)]
struct GroupedTasks {
    title: String,
    children: HashMap<Uuid, ChildRec>,
    done_units: HashMap<TaskCategory, u64>,
    done_count: HashMap<TaskCategory, u64>,
    seen_count: HashMap<TaskCategory, u64>,
    /// Expected counts/bytes announced up-front so totals don't climb mid-download.
    reserved_count: HashMap<TaskCategory, u64>,
    reserved_units: HashMap<TaskCategory, u64>,
    /// Transfer rate and ETA for this group's downloads.
    meter: TransferMeter,
}

impl GroupedTasks {
    /// Aggregate live + finished children into one row per category, in display order.
    fn task_list(&self) -> Vec<TaskView> {
        CATEGORY_ORDER
            .iter()
            .filter_map(|&cat| {
                let seen_count = self.seen_count.get(&cat).copied().unwrap_or(0);
                let reserved_count = self.reserved_count.get(&cat).copied().unwrap_or(0);
                let total_count = seen_count.max(reserved_count);
                if total_count == 0 {
                    return None;
                }
                let done_units = self.done_units.get(&cat).copied().unwrap_or(0);
                let done_count = self.done_count.get(&cat).copied().unwrap_or(0);
                let reserved_units = self.reserved_units.get(&cat).copied().unwrap_or(0);

                let live: Vec<&ChildRec> =
                    self.children.values().filter(|c| c.category == cat).collect();
                let live_current: u64 = live.iter().map(|c| c.current.min(c.total)).sum();
                let live_total: u64 = live.iter().map(|c| c.total).sum();

                // Surface the most advanced phase among live children; default Downloading.
                let phase = live
                    .iter()
                    .map(|c| c.phase)
                    .find(|p| *p != "Downloading")
                    .unwrap_or("Downloading");

                Some(TaskView {
                    label: cat.label().to_string(),
                    phase,
                    current: done_units + live_current,
                    // Reserved total keeps the denominator stable while children are
                    // still being added; fall back to what we've actually seen.
                    total: reserved_units.max(done_units + live_total),
                    done_count,
                    total_count,
                })
            })
            .collect()
    }

    /// Which coarse group is currently downloading, for the notification body.
    fn active_body(&self) -> Option<&'static str> {
        let mut minecraft = false;
        let mut packages = false;
        for child in self.children.values() {
            if child.category.is_minecraft() {
                minecraft = true;
            } else {
                packages = true;
            }
        }
        if minecraft {
            Some("Downloading Minecraft")
        } else if packages {
            Some("Downloading Packages")
        } else {
            None
        }
    }
}

impl NotificationState {
    pub fn snapshot(
        &self,
        inbox: &[InboxEntry],
        center_open: bool,
        pending_prompt: Option<PendingPromptView>,
    ) -> NotificationSnapshot {
        NotificationSnapshot {
            inbox: inbox.to_vec(),
            center_open,
            pending_prompt,
            cluster_update: self.cluster_update.clone(),
            active_toast_entry_ids: self.active_toasts.iter().map(|t| t.entry_id).collect(),
        }
    }

    /// Entry ids with a toast currently showing.
    #[must_use]
    pub fn active_toast_entry_ids(&self) -> Vec<u64> {
        self.active_toasts.iter().map(|t| t.entry_id).collect()
    }

    pub fn open_cluster_update(&mut self, summaries: Vec<ClusterUpdateSummary>) {
        if summaries.is_empty() {
            return;
        }
        self.cluster_update = Some(summaries);
    }

    pub fn close_cluster_update(&mut self) {
        self.cluster_update = None;
    }

    pub fn unread_count(inbox: &[InboxEntry]) -> usize {
        inbox.iter().filter(|entry| !entry.read).count()
    }

    /// Folds one notification into the engine state. Does not build a
    /// [`NotificationSnapshot`]: during a download this runs tens of thousands
    /// of times, and every snapshot deep-clones the whole inbox only for the
    /// caller to discard all but the last. Callers snapshot once, after draining.
    pub fn dispatch(
        &mut self,
        inbox: &mut Vec<InboxEntry>,
        event: Event,
    ) -> (Vec<ToastDismissTimer>, Option<PendingPrompt>) {
        self.pending_timers.clear();

        match event {
            Event::Notification(Notification::Message(message)) => {
                let entry_id = self.push_inbox(
                    inbox,
                    message.title,
                    message.body,
                    message.level,
                    None,
                    false,
                );
                self.push_ephemeral_toast(entry_id, MESSAGE_TOAST_TTL);
            }
            // The Microsoft login renders its own progress in the sign-in
            // modal, so it must not also become a toast. This is a front-end
            // policy decision about a progress id the core merely names.
            Event::Progress(ProgressEvent::Update { id, .. })
                if id == oneclient_auth::MICROSOFT_LOGIN_PROGRESS => {}
            Event::Progress(ProgressEvent::Update {
                id,
                label,
                current,
                total,
            }) => {
                self.handle_progress(inbox, id, label, current, total);
            }
            Event::Progress(ProgressEvent::Complete { id, title, body }) => {
                self.handle_progress_complete(inbox, id, title, body);
            }
            Event::Progress(ProgressEvent::Grouped(event)) => {
                self.handle_grouped_progress(inbox, event);
            }
            // Signals and game events drive the snapshot directly in the
            // runtime loop; they never become inbox entries.
            Event::Signal(_) | Event::Game(_) => {}
            Event::Notification(Notification::Prompt(request)) => {
                let pending_prompt = Some(PendingPrompt {
                    title: request.title,
                    question: request.body,
                    choices: request.choices,
                    dismiss: request.dismiss,
                    reply_tx: Some(request.reply),
                });
                let timers = std::mem::take(&mut self.pending_timers);
                return (timers, pending_prompt);
            }
        }

        let timers = std::mem::take(&mut self.pending_timers);
        (timers, None)
    }

    pub fn toggle_center(&mut self, _inbox: &mut [InboxEntry], center_open: bool) -> bool {
        let next = !center_open;
        if next {
            self.active_toasts.clear();
            self.pending_timers.clear();
        }
        next
    }

    pub fn clear_inbox(&mut self) {
        self.progress_entries.clear();
        self.grouped_entries.clear();
        self.grouped_tasks.clear();
        self.active_toasts.clear();
        self.pending_timers.clear();
    }

    pub fn dismiss_toast(&mut self, inbox: &mut Vec<InboxEntry>, entry_id: u64) {
        let Some(pos) = self
            .active_toasts
            .iter()
            .position(|t| t.entry_id == entry_id)
        else {
            return;
        };
        let entry = inbox.iter().find(|e| e.id == entry_id);
        if entry.is_some_and(|e| !e.dismissable()) {
            return;
        }
        let has_progress = entry.is_some_and(|e| e.progress.is_some());
        self.active_toasts.remove(pos);
        if !has_progress {
            self.forget_entry(inbox, entry_id);
        }
    }

    #[allow(dead_code)]
    pub fn dismiss_toast_timer(&mut self, _inbox: &[InboxEntry], toast_id: u64, _entry_id: u64) {
        self.active_toasts
            .retain(|toast| toast.toast_id != toast_id);
    }

    pub fn expire_toast(&mut self, _inbox: &[InboxEntry], entry_id: u64) {
        self.active_toasts
            .retain(|toast| toast.entry_id != entry_id);
    }

    pub fn mark_read(&mut self, inbox: &mut [InboxEntry], entry_id: u64) {
        if let Some(entry) = inbox.iter_mut().find(|e| e.id == entry_id) {
            entry.read = true;
        }
    }

    pub fn dismiss_notification(&mut self, inbox: &mut Vec<InboxEntry>, entry_id: u64) {
        if inbox
            .iter()
            .find(|e| e.id == entry_id)
            .is_some_and(|e| e.dismissable())
        {
            self.forget_entry(inbox, entry_id);
        }
    }

    fn forget_entry(&mut self, inbox: &mut Vec<InboxEntry>, entry_id: u64) {
        inbox.retain(|e| e.id != entry_id);
        self.active_toasts.retain(|t| t.entry_id != entry_id);
        self.progress_entries.retain(|_, &mut v| v != entry_id);
        self.grouped_entries.retain(|_, &mut v| v != entry_id);
    }

    fn push_inbox(
        &mut self,
        inbox: &mut Vec<InboxEntry>,
        title: String,
        body: String,
        level: Level,
        progress: Option<(u64, u64)>,
        is_loading: bool,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        inbox.insert(
            0,
            InboxEntry {
                id,
                title,
                body,
                level,
                icon: None,
                progress,
                is_loading,
                read: false,
                created_at: Instant::now(),
                actions: Vec::new(),
                tasks: Vec::new(),
                transfer: None,
            },
        );
        id
    }

    pub fn push_custom(&mut self, inbox: &mut Vec<InboxEntry>, spec: NotificationSpec) -> u64 {
        let NotificationSpec {
            title,
            body,
            level,
            icon,
            progress,
            actions,
        } = spec;

        let is_loading = progress.is_some_and(|(current, total)| total == 0 || current < total);

        let id = self.next_id;
        self.next_id += 1;
        inbox.insert(
            0,
            InboxEntry {
                id,
                title,
                body,
                level,
                icon,
                progress: progress.map(|(current, total)| (current, total.max(1))),
                is_loading,
                read: false,
                created_at: Instant::now(),
                actions,
                tasks: Vec::new(),
                transfer: None,
            },
        );

        if progress.is_some() {
            self.ensure_progress_toast(id);
        } else {
            self.push_ephemeral_toast(id, MESSAGE_TOAST_TTL);
        }

        id
    }

    fn update_inbox_entry(
        &self,
        inbox: &mut [InboxEntry],
        entry_id: u64,
        title: impl Into<String>,
        body: impl Into<String>,
        progress: Option<(u64, u64)>,
        is_loading: bool,
    ) {
        if let Some(entry) = inbox.iter_mut().find(|e| e.id == entry_id) {
            entry.title = title.into();
            entry.body = body.into();
            entry.progress = progress;
            entry.is_loading = is_loading;
            entry.read = false;
        }
    }

    fn push_ephemeral_toast(&mut self, entry_id: u64, ttl: Duration) {
        let toast_id = self.next_id;
        self.next_id += 1;
        self.active_toasts.push(ActiveToast { toast_id, entry_id });
        self.pending_timers.push(ToastDismissTimer {
            toast_id,
            entry_id,
            after: ttl,
        });
    }

    fn ensure_progress_toast(&mut self, entry_id: u64) {
        if self
            .active_toasts
            .iter()
            .any(|toast| toast.entry_id == entry_id)
        {
            return;
        }

        let toast_id = self.next_id;
        self.next_id += 1;
        self.active_toasts.push(ActiveToast { toast_id, entry_id });
    }

    fn handle_progress(
        &mut self,
        inbox: &mut Vec<InboxEntry>,
        id: Uuid,
        label: String,
        current: u64,
        total: u64,
    ) {
        let done = total > 0 && current >= total;
        let body = progress_body(&label, current, total);
        let progress = Some((current, total.max(1)));

        let entry_id = if let Some(&entry_id) = self.progress_entries.get(&id) {
            entry_id
        } else {
            let entry_id = self.push_inbox(
                inbox,
                label.clone(),
                body.clone(),
                Level::Info,
                progress,
                !done,
            );
            self.progress_entries.insert(id, entry_id);
            entry_id
        };

        self.update_inbox_entry(inbox, entry_id, label, body, progress, !done);
        self.ensure_progress_toast(entry_id);

        // The id->entry mapping is kept even once `done`, so a later
        // `ProgressComplete` can convert this same card into its finished state (see
        // `handle_progress_complete`). It is cleaned up when the entry is dismissed
        // or the engine is reset.
    }

    /// Convert an in-flight progress card into a finished message in place. Used so an
    /// operation and its "done" notice stay a single notification. Falls back to a fresh
    /// message if the progress entry is already gone (e.g. dismissed mid-flight).
    fn handle_progress_complete(
        &mut self,
        inbox: &mut Vec<InboxEntry>,
        id: Uuid,
        title: String,
        body: String,
    ) {
        if let Some(entry_id) = self.progress_entries.remove(&id) {
            self.update_inbox_entry(inbox, entry_id, title, body, None, false);
            self.ensure_progress_toast(entry_id);
        } else {
            let entry_id =
                self.push_inbox(inbox, title, body, Level::Info, None, false);
            self.push_ephemeral_toast(entry_id, MESSAGE_TOAST_TTL);
        }
    }

    fn handle_grouped_progress(
        &mut self,
        inbox: &mut Vec<InboxEntry>,
        event: GroupedProgressEvent,
    ) {
        match event {
            GroupedProgressEvent::Start { session_id, title } => {
                let entry_id = self.push_inbox(
                    inbox,
                    title.clone(),
                    "Preparing...".to_string(),
                    Level::Info,
                    None,
                    true,
                );
                self.grouped_entries.insert(session_id, entry_id);
                self.grouped_tasks.insert(
                    session_id,
                    GroupedTasks {
                        title,
                        ..Default::default()
                    },
                );
            }
            GroupedProgressEvent::Expect {
                session_id,
                category,
                count,
                total,
            } => {
                if let Some(group) = self.grouped_tasks.get_mut(&session_id) {
                    *group.reserved_count.entry(category).or_default() += count;
                    *group.reserved_units.entry(category).or_default() += total;
                }
                self.refresh_grouped_entry(inbox, session_id);
                if let Some(&entry_id) = self.grouped_entries.get(&session_id) {
                    self.ensure_progress_toast(entry_id);
                }
            }
            GroupedProgressEvent::AddChild {
                session_id,
                child_id,
                label: _,
                total,
                category,
            } => {
                if let Some(group) = self.grouped_tasks.get_mut(&session_id) {
                    group.children.insert(
                        child_id,
                        ChildRec {
                            category,
                            phase: "Downloading",
                            current: 0,
                            total: total.max(1),
                        },
                    );
                    *group.seen_count.entry(category).or_default() += 1;
                }
                self.refresh_grouped_entry(inbox, session_id);
                if let Some(&entry_id) = self.grouped_entries.get(&session_id) {
                    self.ensure_progress_toast(entry_id);
                }
            }
            GroupedProgressEvent::UpdateChild {
                session_id,
                child_id,
                current,
                total,
            } => {
                if let Some(group) = self.grouped_tasks.get_mut(&session_id)
                    && let Some(task) = group.children.get_mut(&child_id)
                {
                    task.current = current;
                    task.total = total.max(1);
                }
                self.refresh_grouped_entry(inbox, session_id);
            }
            GroupedProgressEvent::SetChildPhase {
                session_id,
                child_id,
                phase,
            } => {
                if let Some(group) = self.grouped_tasks.get_mut(&session_id)
                    && let Some(task) = group.children.get_mut(&child_id)
                {
                    task.phase = phase.label();
                }
                self.refresh_grouped_entry(inbox, session_id);
            }
            GroupedProgressEvent::FinishChild {
                session_id,
                child_id,
            } => {
                if let Some(group) = self.grouped_tasks.get_mut(&session_id)
                    && let Some(task) = group.children.remove(&child_id)
                {
                    *group.done_units.entry(task.category).or_default() += task.total;
                    *group.done_count.entry(task.category).or_default() += 1;
                }
                self.refresh_grouped_entry(inbox, session_id);
            }
            GroupedProgressEvent::End { session_id } => {
                self.grouped_tasks.remove(&session_id);
                if let Some(entry_id) = self.grouped_entries.remove(&session_id) {
                    let had_progress = inbox
                        .iter()
                        .find(|e| e.id == entry_id)
                        .is_some_and(|e| e.progress.is_some());
                    if !had_progress {
                        self.forget_entry(inbox, entry_id);
                    } else if let Some(entry) = inbox.iter_mut().find(|e| e.id == entry_id) {
                        entry.body = "Complete".to_string();
                        entry.progress = Some((1, 1));
                        entry.is_loading = false;
                        entry.tasks = Vec::new();
                        entry.transfer = None;
                    }
                }
            }
        }
    }

    /// Converts a still-open grouped-progress entry into its finished state,
    /// reusing the same inbox id. `spec: None` forgets the entry entirely (used
    /// when a sync completed with nothing to show). This lets a download progress
    /// notification morph into a "view changes" notification without ever
    /// becoming a second entry.
    pub fn finish_grouped_as_actions(
        &mut self,
        inbox: &mut Vec<InboxEntry>,
        session_id: Uuid,
        spec: Option<NotificationSpec>,
    ) {
        self.grouped_tasks.remove(&session_id);
        let entry_id = self.grouped_entries.remove(&session_id);

        let Some(spec) = spec else {
            // Nothing changed: drop the progress entry if one was created.
            if let Some(entry_id) = entry_id {
                self.forget_entry(inbox, entry_id);
            }
            return;
        };

        let NotificationSpec {
            title,
            body,
            level,
            icon,
            progress: _,
            actions,
        } = spec;

        match entry_id.and_then(|id| inbox.iter_mut().find(|e| e.id == id)) {
            Some(entry) => {
                entry.title = title;
                entry.body = body;
                entry.level = level;
                entry.icon = icon;
                entry.progress = None;
                entry.is_loading = false;
                entry.read = false;
                entry.created_at = Instant::now();
                entry.actions = actions;
                entry.tasks = Vec::new();
                entry.transfer = None;
                // The progress toast (if any) is already armed for this entry;
                // leaving it in `active_toasts` lets the loop give it a dismiss
                // timer now that it is no longer loading.
                self.ensure_progress_toast(entry.id);
            }
            None => {
                // Progress entry never materialised (e.g. no downloads happened);
                // fall back to a fresh notification so the result still shows.
                self.push_custom(
                    inbox,
                    NotificationSpec {
                        title,
                        body,
                        level,
                        icon,
                        progress: None,
                        actions,
                    },
                );
            }
        }
    }

    fn refresh_grouped_entry(&mut self, inbox: &mut [InboxEntry], session_id: Uuid) {
        let Some(&entry_id) = self.grouped_entries.get(&session_id) else {
            return;
        };
        let Some(group) = self.grouped_tasks.get_mut(&session_id) else {
            return;
        };

        let tasks = group.task_list();
        let body = group
            .active_body()
            .map(str::to_string)
            .unwrap_or_else(|| "Preparing...".to_string());
        let title = group.title.clone();

        let completed: u64 = tasks.iter().map(|t| t.current.min(t.total)).sum();
        let total: u64 = tasks.iter().map(|t| t.total).sum::<u64>().max(1);

        // Below 1 B/s the estimate is noise, not information.
        let transfer = group
            .meter
            .sample(completed, total)
            .filter(|stats| stats.speed_bps >= 1.0);

        let Some(entry) = inbox.iter_mut().find(|e| e.id == entry_id) else {
            return;
        };
        entry.title = title;
        entry.body = body;
        entry.progress = Some((completed, total));
        entry.is_loading = true;
        entry.read = false;
        entry.tasks = tasks;
        entry.transfer = transfer;
    }
}

fn progress_body(label: &str, current: u64, total: u64) -> String {
    if total == 0 {
        return label.to_string();
    }

    let percent = ((current as f64 / total as f64) * 100.0).round() as u64;
    format!("{label} - {percent}%")
}
