use reqwest::Response;
use uuid::Uuid;

use crate::error::RequestError;
use oneclient_events::{GroupedProgressChild, EventBus};

/// Minimum gap between progress events emitted for a single response body.
const PROGRESS_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

#[derive(Clone, Default)]
pub struct ResponseOptions {
    pub notify: Option<ResponseNotifyOptions>,
}

#[derive(Clone)]
pub struct ResponseNotifyOptions {
    child: Option<GroupedProgressChild>,
    standalone_label: Option<String>,
    standalone_id: Option<Uuid>,
    done_label: Option<String>,
}

impl ResponseNotifyOptions {
    pub fn grouped(child: GroupedProgressChild) -> Self {
        Self {
            child: Some(child),
            standalone_label: None,
            standalone_id: None,
            done_label: None,
        }
    }

    pub fn standalone(label: impl Into<String>) -> Self {
        Self {
            child: None,
            standalone_label: Some(label.into()),
            standalone_id: None,
            done_label: None,
        }
    }

    pub fn with_id(mut self, id: Uuid) -> Self {
        self.standalone_id = Some(id);
        self
    }

    pub fn done_label(mut self, label: impl Into<String>) -> Self {
        self.done_label = Some(label.into());
        self
    }

    /// The grouped child these options report against, if any. Lets a caller
    /// driving the download set phases and finish the child without having to
    /// keep a second copy of it alongside the options.
    #[must_use]
    pub fn child(&self) -> Option<&GroupedProgressChild> {
        self.child.as_ref()
    }

    /// Fixes the standalone notification's id, so a caller that sends the same
    /// request more than once keeps updating one notification.
    ///
    /// Without this a retried download opens a fresh entry in the UI on every
    /// attempt, and a flaky connection reads as several stalled installs rather
    /// than one that is recovering.
    #[must_use]
    pub fn pinned(mut self) -> Self {
        if self.standalone_label.is_some() && self.standalone_id.is_none() {
            self.standalone_id = Some(Uuid::new_v4());
        }
        self
    }
}

#[async_trait::async_trait]
pub trait ResponseExt {
    async fn stream(
        self,
        options: ResponseOptions,
        events: &EventBus,
    ) -> Result<impl futures_lite::Stream<Item = Result<bytes::Bytes, RequestError>>, RequestError>;
}

#[async_trait::async_trait]
impl ResponseExt for Response {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn stream(
        self,
        options: ResponseOptions,
        events: &EventBus,
    ) -> Result<impl futures_lite::Stream<Item = Result<bytes::Bytes, RequestError>>, RequestError>
    {
        let total = self.content_length().unwrap_or(0).max(1);
        let mut current = 0u64;

        let grouped_child = options
            .notify
            .as_ref()
            .and_then(|n| n.child.clone());
        let standalone_label = options
            .notify
            .as_ref()
            .and_then(|n| n.standalone_label.clone());
        let done_label = options
            .notify
            .as_ref()
            .and_then(|n| n.done_label.clone());

        let standalone_id = standalone_label
            .as_ref()
            .map(|_| options.notify.as_ref().and_then(|n| n.standalone_id).unwrap_or_else(Uuid::new_v4));

        if let Some(ref child) = grouped_child {
            child.set_progress(0, Some(total));
        } else if let (Some(id), Some(label)) = (&standalone_id, &standalone_label) {
            events.progress(*id, label, 0, total);
        }

        let events = events.clone();
        // A chunk arrives every few KiB; forwarding one event per chunk buries the
        // UI in tens of thousands of updates per download session, all of which
        // collapse into the same repainted progress bar. Sample instead, and
        // always emit the last one so the bar lands on complete.
        let mut last_emit: Option<std::time::Instant> = None;
        let stream = futures_lite::StreamExt::map(self.bytes_stream(), move |item| {
            match item {
                Ok(chunk) => {
                    current += chunk.len() as u64;

                    let now = std::time::Instant::now();
                    let due = current >= total
                        || last_emit.is_none_or(|last| now.duration_since(last) >= PROGRESS_INTERVAL);

                    if due {
                        last_emit = Some(now);
                        if let Some(ref child) = grouped_child {
                            child.set_progress(current, Some(total));
                        } else if let (Some(id), Some(label)) = (&standalone_id, &standalone_label) {
                            let done = current >= total;
                            let label = if done {
                                done_label.as_deref().unwrap_or(label)
                            } else {
                                label
                            };
                            events.progress(*id, label, current, total);
                        }
                    }

                    Ok(chunk)
                }
                Err(err) => Err(RequestError::from(err)),
            }
        });

        Ok(stream)
    }
}
