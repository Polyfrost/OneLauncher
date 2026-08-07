//! Both handles are RAII dropping the last one ends the session or finishes
//! the child so an early return on an error path cannot leave the UI showing a
//! task that will never complete

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use uuid::Uuid;

use crate::bus::EventBus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupedProgressEvent {
    Start {
        session_id: Uuid,
        title: String,
    },
    /// Pre-announces a category's children so the aggregate total is known
    /// up-front instead of climbing during a `buffer_unordered` fan-out
    Expect {
        session_id: Uuid,
        category: TaskCategory,
        count: u64,
        total: u64,
    },
    AddChild {
        session_id: Uuid,
        child_id: Uuid,
        label: String,
        total: u64,
        category: TaskCategory,
    },
    UpdateChild {
        session_id: Uuid,
        child_id: Uuid,
        current: u64,
        total: u64,
    },
    SetChildPhase {
        session_id: Uuid,
        child_id: Uuid,
        phase: TaskPhase,
    },
    FinishChild {
        session_id: Uuid,
        child_id: Uuid,
    },
    End {
        session_id: Uuid,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskPhase {
    #[default]
    Downloading,
    Verifying,
    Extracting,
    Installing,
}

impl TaskPhase {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Downloading => "Downloading",
            Self::Verifying => "Verifying",
            Self::Extracting => "Extracting",
            Self::Installing => "Installing",
        }
    }
}

/// Display labels only not a dependency on the subsystems they name
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TaskCategory {
    Client,
    Metadata,
    Libraries,
    Natives,
    Assets,
    Java,
    #[default]
    Packages,
}

impl TaskCategory {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Client => "Client",
            Self::Metadata => "Metadata",
            Self::Libraries => "Libraries",
            Self::Natives => "Natives",
            Self::Assets => "Assets",
            Self::Java => "Java runtime",
            Self::Packages => "Packages",
        }
    }

    #[must_use]
    pub fn is_minecraft(self) -> bool {
        !matches!(self, Self::Packages)
    }
}

struct SessionInner {
    session_id: Uuid,
    events: EventBus,
    ended: AtomicBool,
}

impl SessionInner {
    fn end(&self) {
        if self.ended.swap(true, Ordering::Relaxed) {
            return;
        }

        self.events.emit(GroupedProgressEvent::End {
            session_id: self.session_id,
        });
    }
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        self.end();
    }
}

struct ChildInner {
    session_id: Uuid,
    child_id: Uuid,
    events: EventBus,
    label: String,
    total: AtomicU64,
    finished: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
pub struct GroupedProgressSession {
    inner: Arc<SessionInner>,
}

#[derive(Clone)]
pub struct GroupedProgressChild {
    inner: Arc<ChildInner>,
}

impl GroupedProgressSession {
    pub fn start(events: &EventBus, title: impl Into<String>) -> Self {
        let session_id = Uuid::new_v4();
        let title = title.into();

        events.emit(GroupedProgressEvent::Start { session_id, title });

        Self {
            inner: Arc::new(SessionInner {
                session_id,
                events: events.clone(),
                ended: AtomicBool::new(false),
            }),
        }
    }

    pub fn id(&self) -> Uuid {
        self.inner.session_id
    }

    /// `count` = number of files `total` = sum of their sizes in bytes
    pub fn expect(&self, category: TaskCategory, count: u64, total: u64) {
        if count == 0 {
            return;
        }
        self.inner
            .events
            .emit(GroupedProgressEvent::Expect {
                session_id: self.inner.session_id,
                category,
                count,
                total,
            });
    }

    pub fn child(
        &self,
        label: impl Into<String>,
        total: u64,
        category: TaskCategory,
    ) -> GroupedProgressChild {
        let child_id = Uuid::new_v4();
        let label = label.into();
        let total = total.max(1);

        self.inner
            .events
            .emit(GroupedProgressEvent::AddChild {
                session_id: self.inner.session_id,
                child_id,
                label: label.clone(),
                total,
                category,
            });

        GroupedProgressChild {
            inner: Arc::new(ChildInner {
                session_id: self.inner.session_id,
                child_id,
                events: self.inner.events.clone(),
                label,
                total: AtomicU64::new(total),
                finished: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }),
        }
    }

    pub async fn run_child<T, E, F, Fut>(
        &self,
        label: impl Into<String>,
        total: u64,
        category: TaskCategory,
        f: F,
    ) -> Result<T, E>
    where
        F: FnOnce(GroupedProgressChild) -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let child = self.child(label, total, category);
        let result = f(child.clone()).await;
        if result.is_ok() {
            child.finish();
        }
        result
    }

    pub fn finish(self) {
        self.inner.end();
    }

    /// Consumes the session without emitting an `End` event
    /// Use when another actor will take over the session's notification entry
    pub fn detach(self) -> Uuid {
        self.inner.ended.store(true, Ordering::Relaxed);
        self.inner.session_id
    }
}

impl GroupedProgressChild {
    pub fn label(&self) -> &str {
        &self.inner.label
    }

    pub fn set_phase(&self, phase: TaskPhase) {
        if self
            .inner
            .finished
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        self.inner
            .events
            .emit(GroupedProgressEvent::SetChildPhase {
                session_id: self.inner.session_id,
                child_id: self.inner.child_id,
                phase,
            });
    }

    pub fn set_progress(&self, current: u64, total: Option<u64>) {
        if self
            .inner
            .finished
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }

        // Prefer the largest known total Content-Length is often missing (1) for
        // chunked responses and would clobber a real manifest size freezing the bar
        let stored = self.inner.total.load(Ordering::Relaxed);
        let total = total.unwrap_or(0).max(stored).max(1);
        if total > stored {
            self.inner.total.store(total, Ordering::Relaxed);
        }
        self.inner
            .events
            .emit(GroupedProgressEvent::UpdateChild {
                session_id: self.inner.session_id,
                child_id: self.inner.child_id,
                current,
                total,
            });
    }

    pub fn finish(&self) {
        if self
            .inner
            .finished
            .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }

        self.inner
            .events
            .emit(GroupedProgressEvent::FinishChild {
                session_id: self.inner.session_id,
                child_id: self.inner.child_id,
            });
    }
}

impl Drop for GroupedProgressChild {
    /// Only the *last* handle finishes the task clones are handed around
    /// routinely and finishing on any clone's drop would end the task early
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, ProgressEvent};

    fn drain(rx: &mut crate::bus::EventReceiver) -> Vec<GroupedProgressEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let Event::Progress(ProgressEvent::Grouped(event)) = event {
                events.push(event);
            }
        }
        events
    }

    fn has_finish(events: &[GroupedProgressEvent]) -> bool {
        events
            .iter()
            .any(|e| matches!(e, GroupedProgressEvent::FinishChild { .. }))
    }

    #[test]
    fn dropping_a_clone_does_not_finish_the_task() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let session = GroupedProgressSession::start(&events, "test");
        let child = session.child("file", 100, TaskCategory::Assets);
        let _ = drain(&mut rx);

        drop(child.clone());
        assert!(
            !has_finish(&drain(&mut rx)),
            "a cloned handle going out of scope must not finish the task"
        );

        child.set_progress(50, Some(100));
        let events = drain(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                GroupedProgressEvent::UpdateChild { current: 50, .. }
            )),
            "progress must still be reported after a clone is dropped: {events:?}"
        );

        drop(child);
        assert!(
            has_finish(&drain(&mut rx)),
            "the last handle going out of scope should finish the task"
        );
    }

    #[test]
    fn explicit_finish_reports_once() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let session = GroupedProgressSession::start(&events, "test");
        let child = session.child("file", 100, TaskCategory::Assets);
        let _ = drain(&mut rx);

        child.finish();
        assert_eq!(drain(&mut rx).iter().filter(|e| matches!(e, GroupedProgressEvent::FinishChild { .. })).count(), 1);

        drop(child);
        assert!(!has_finish(&drain(&mut rx)), "drop must not finish twice");
    }

    #[test]
    fn progress_after_finish_is_ignored() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let session = GroupedProgressSession::start(&events, "test");
        let child = session.child("file", 100, TaskCategory::Assets);
        child.finish();
        let _ = drain(&mut rx);

        child.set_progress(75, Some(100));
        assert!(drain(&mut rx).is_empty());
    }
}
