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
}

impl ResponseNotifyOptions {
    pub fn grouped(child: GroupedProgressChild) -> Self {
        Self {
            child: Some(child),
            standalone_label: None,
        }
    }

    pub fn standalone(label: impl Into<String>) -> Self {
        Self {
            child: None,
            standalone_label: Some(label.into()),
        }
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
        let standalone_id = standalone_label.as_ref().map(|_| Uuid::new_v4());

        if let Some(ref child) = grouped_child {
            child.set_progress(0, Some(total));
        } else if let (Some(id), Some(label)) = (&standalone_id, &standalone_label) {
            notifier.send_progress(id, label, 0, total);
        }

        let stream = futures_lite::StreamExt::map(self.bytes_stream(), move |item| {
            match item {
                Ok(chunk) => {
                    current += chunk.len() as u64;

                    if let Some(ref child) = grouped_child {
                        child.set_progress(current, Some(total));
                    } else if let (Some(id), Some(label)) = (&standalone_id, &standalone_label) {
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
