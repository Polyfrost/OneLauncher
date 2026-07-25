use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use uuid::Uuid;

use super::NotificationService;
use super::data::{GroupedProgressEvent, TaskCategory};

struct SessionInner {
    session_id: Uuid,
    notifier: NotificationService,
    ended: AtomicBool,
}

impl SessionInner {
    fn end(&self) {
        if self.ended.swap(true, Ordering::Relaxed) {
            return;
        }

        self.notifier.send_grouped(GroupedProgressEvent::End {
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
    notifier: NotificationService,
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
    pub fn start(notifier: &NotificationService, title: impl Into<String>) -> Self {
        let session_id = Uuid::new_v4();
        let title = title.into();

        notifier.send_grouped(GroupedProgressEvent::Start { session_id, title });

        Self {
            inner: Arc::new(SessionInner {
                session_id,
                notifier: notifier.clone(),
                ended: AtomicBool::new(false),
            }),
        }
    }

    pub fn id(&self) -> Uuid {
        self.inner.session_id
    }

    /// Reserve the expected work for a category before its children are added.
    /// `count` = number of files, `total` = sum of their sizes in bytes.
    pub fn expect(&self, category: TaskCategory, count: u64, total: u64) {
        if count == 0 {
            return;
        }
        self.inner
            .notifier
            .send_grouped(GroupedProgressEvent::Expect {
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
            .notifier
            .send_grouped(GroupedProgressEvent::AddChild {
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
                notifier: self.inner.notifier.clone(),
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

    /// Consumes the session without emitting an `End` event, returning its id.
    /// Use when another actor (e.g. the UI bridge) will take over the session's
    /// notification entry and convert it to a finished state itself.
    pub fn detach(self) -> Uuid {
        self.inner.ended.store(true, Ordering::Relaxed);
        self.inner.session_id
    }
}

impl GroupedProgressChild {
    pub fn label(&self) -> &str {
        &self.inner.label
    }

    pub fn set_phase(&self, phase: super::data::TaskPhase) {
        if self
            .inner
            .finished
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
        self.inner
            .notifier
            .send_grouped(GroupedProgressEvent::SetChildPhase {
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

        // Prefer the largest known total. Streaming reports Content-Length which is
        // often missing (1) for compressed/chunked responses; when the child was
        // created with a real expected size (from a manifest) we must not let that
        // stale header clobber it — otherwise the download bar reads 100%/frozen.
        let stored = self.inner.total.load(Ordering::Relaxed);
        let total = total.unwrap_or(0).max(stored).max(1);
        if total > stored {
            self.inner.total.store(total, Ordering::Relaxed);
        }
        self.inner
            .notifier
            .send_grouped(GroupedProgressEvent::UpdateChild {
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
            .notifier
            .send_grouped(GroupedProgressEvent::FinishChild {
                session_id: self.inner.session_id,
                child_id: self.inner.child_id,
            });
    }
}

impl Drop for GroupedProgressChild {
    /// Only the *last* handle finishes the task.
    ///
    /// `finished` lives in the shared inner, so finishing on every clone's drop
    /// ended the task the moment any temporary copy died — and copies are handed
    /// around routinely (into `ResponseOptions`, into stream closures). A
    /// download would emit `FinishChild` before its first byte arrived, after
    /// which every `set_progress` silently no-opped and the UI removed the row.
    /// The safety net for early returns still works: on an error path the last
    /// handle goes out of scope too.
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notification::Notification;

    fn drain(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<Notification>,
    ) -> Vec<GroupedProgressEvent> {
        let mut events = Vec::new();
        while let Ok(notification) = rx.try_recv() {
            if let Notification::GroupedProgress(event) = notification {
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

    /// A child handle gets cloned into `ResponseOptions` and into the streaming
    /// closure. If dropping one of those copies finished the task, the download
    /// would report completion before its first byte and every later progress
    /// update would be dropped on the floor.
    #[test]
    fn dropping_a_clone_does_not_finish_the_task() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let notifier = NotificationService::new(tx);
        let session = GroupedProgressSession::start(&notifier, "test");
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
        let notifier = NotificationService::new(tx);
        let session = GroupedProgressSession::start(&notifier, "test");
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
        let notifier = NotificationService::new(tx);
        let session = GroupedProgressSession::start(&notifier, "test");
        let child = session.child("file", 100, TaskCategory::Assets);
        child.finish();
        let _ = drain(&mut rx);

        child.set_progress(75, Some(100));
        assert!(drain(&mut rx).is_empty());
    }
}
