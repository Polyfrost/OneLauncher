use std::sync::Arc;

use uuid::Uuid;

use super::data::GroupedProgressEvent;
use super::NotificationService;

#[derive(Clone)]
struct SessionInner {
    session_id: Uuid,
    notifier: NotificationService,
}

#[derive(Clone)]
struct ChildInner {
    session_id: Uuid,
    child_id: Uuid,
    notifier: NotificationService,
    label: String,
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

    pub fn child(&self, label: impl Into<String>, total: u64) -> GroupedProgressChild {
        let child_id = Uuid::new_v4();
        let label = label.into();
        let total = total.max(1);

        self.inner.notifier.send_grouped(GroupedProgressEvent::AddChild {
            session_id: self.inner.session_id,
            child_id,
            label: label.clone(),
            total,
        });

        GroupedProgressChild {
            inner: Arc::new(ChildInner {
                session_id: self.inner.session_id,
                child_id,
                notifier: self.inner.notifier.clone(),
                label,
                finished: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }),
        }
    }

    pub async fn run_child<T, E, F, Fut>(
        &self,
        label: impl Into<String>,
        total: u64,
        f: F,
    ) -> Result<T, E>
    where
        F: FnOnce(GroupedProgressChild) -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let child = self.child(label, total);
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

        let total = total.unwrap_or(1).max(1);
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

