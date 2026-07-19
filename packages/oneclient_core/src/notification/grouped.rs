use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use uuid::Uuid;

use super::data::{GroupedProgressEvent, TaskCategory};
use super::NotificationService;

#[derive(Clone)]
struct SessionInner {
    session_id: Uuid,
    notifier: NotificationService,
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
        self.inner.notifier.send_grouped(GroupedProgressEvent::Expect {
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

        self.inner.notifier.send_grouped(GroupedProgressEvent::AddChild {
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
        self.inner
            .notifier
            .send_grouped(GroupedProgressEvent::End {
                session_id: self.inner.session_id,
            });
    }
}

impl GroupedProgressChild {
    pub fn label(&self) -> &str {
        &self.inner.label
    }

    pub fn set_phase(&self, phase: super::data::TaskPhase) {
        if self.inner.finished.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        self.inner.notifier.send_grouped(GroupedProgressEvent::SetChildPhase {
            session_id: self.inner.session_id,
            child_id: self.inner.child_id,
            phase,
        });
    }

    pub fn set_progress(&self, current: u64, total: Option<u64>) {
        if self.inner.finished.load(std::sync::atomic::Ordering::Relaxed) {
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
        self.inner.notifier.send_grouped(GroupedProgressEvent::UpdateChild {
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

        self.inner.notifier.send_grouped(GroupedProgressEvent::FinishChild {
            session_id: self.inner.session_id,
            child_id: self.inner.child_id,
        });
    }
}

impl Drop for GroupedProgressChild {
    fn drop(&mut self) {
        self.finish();
    }
}

