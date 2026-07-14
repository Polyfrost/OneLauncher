use reqwest::Response;
use uuid::Uuid;

use crate::http::RequestError;
use crate::notification::{GroupedProgressChild, NotificationService};

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
}

#[async_trait::async_trait]
pub trait ResponseExt {
    async fn stream(
        self,
        options: ResponseOptions,
        notifier: &NotificationService,
    ) -> Result<impl futures_lite::Stream<Item = Result<bytes::Bytes, RequestError>>, RequestError>;
}

#[async_trait::async_trait]
impl ResponseExt for Response {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn stream(
        self,
        options: ResponseOptions,
        notifier: &NotificationService,
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
            notifier.send_progress(id, label, 0, total);
        }

        let notifier = notifier.clone();
        let stream = futures_lite::StreamExt::map(self.bytes_stream(), move |item| {
            match item {
                Ok(chunk) => {
                    current += chunk.len() as u64;

                    if let Some(ref child) = grouped_child {
                        child.set_progress(current, Some(total));
                    } else if let (Some(id), Some(label)) = (&standalone_id, &standalone_label) {
                        let done = current >= total;
                        let label = if done {
                            done_label.as_deref().unwrap_or(label)
                        } else {
                            label
                        };
                        notifier.send_progress(id, label, current, total);
                    }

                    Ok(chunk)
                }
                Err(err) => Err(RequestError::from(err)),
            }
        });

        Ok(stream)
    }
}
