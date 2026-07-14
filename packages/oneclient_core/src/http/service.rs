use std::sync::Arc;

use reqwest::{ClientBuilder, Response};
use serde::de::DeserializeOwned;
use tokio::sync::Semaphore;

use crate::{LauncherServices, http::{HttpRequest, RequestError, ResponseExt, ResponseOptions}};

const MAX_THROTTLE_RETRIES: u32 = 6;

fn retry_after(response: &Response) -> Option<std::time::Duration> {
    let raw = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .to_string();
    let secs: u64 = raw.parse().ok()?;
    Some(std::time::Duration::from_secs(secs.min(60)))
}

fn backoff_delay(attempt: u32) -> std::time::Duration {
    let base = 500u64.saturating_mul(1 << attempt.min(6));
    let capped = base.min(30_000);
    let jitter = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis() as u64)
        .unwrap_or(0))
        % 250;
    std::time::Duration::from_millis(capped + jitter)
}

fn body_snippet(bytes: &[u8]) -> String {
    const MAX: usize = 240;
    if bytes.is_empty() {
        return "<empty body>".to_string();
    }
    let text = String::from_utf8_lossy(bytes);
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return "<non-text body>".to_string();
    }
    if collapsed.len() > MAX {
        let end = collapsed
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|i| *i <= MAX)
            .last()
            .unwrap_or(0);
        format!("{}...", &collapsed[..end])
    } else {
        collapsed
    }
}

#[derive(Clone)]
pub struct RequestClient {
    client: reqwest::Client,
    semaphore: Arc<Semaphore>,
}

impl RequestClient {
    pub fn http(&self) -> &reqwest::Client {
        &self.client
    }

    pub fn new() -> Result<Self, RequestError> {
        let client = ClientBuilder::new()
            .hickory_dns(true)
            .connect_timeout(std::time::Duration::from_secs(10))
            .read_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_mins(10))
            .tls_backend_rustls()
            .user_agent(format!(
                "OneClient {} ({})",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_HOMEPAGE")
            ))
            .build()?;

        Ok(Self {
            client,
            semaphore: Arc::new(Semaphore::new(12)),
        })
    }
}

impl RequestClient {
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn send(&self, request: impl Into<HttpRequest>) -> Result<Response, RequestError> {
        let request: HttpRequest = request.into();
        let mut retries = 0;
        let mut throttle_retries = 0u32;

        let cloned_backup = request.request.try_clone();
        let cloneable = cloned_backup.is_some();

        let max_retries = if cloneable {
            request.options.max_retries
        } else {
            0
        };

        let mut active_request = request.request;

        tracing::debug!(
            method = %active_request.method(),
            url = %active_request.url(),
            "dispatching http request"
        );

        let res = loop {
            let permit = if request.options.use_semaphore {
                self.semaphore.acquire().await.ok()
            } else {
                None
            };

            match self.client.execute(active_request).await {
                Ok(response) => {
                    let status = response.status();
                    let retryable = status.as_u16() == 429
                        || matches!(status.as_u16(), 502..=504);

                    if retryable && cloneable && throttle_retries < MAX_THROTTLE_RETRIES {
                        throttle_retries += 1;
                        let delay = retry_after(&response)
                            .unwrap_or_else(|| backoff_delay(throttle_retries));
                        tracing::warn!(
                            status = status.as_u16(),
                            attempt = throttle_retries,
                            delay_ms = delay.as_millis() as u64,
                            url = %response.url(),
                            "rate limited / transient error; backing off"
                        );
                        drop(permit);
                        tokio::time::sleep(delay).await;
                        active_request = cloned_backup.as_ref().unwrap().try_clone().unwrap();
                        continue;
                    }

                    break response;
                }
                Err(err) => {
                    if retries < max_retries {
                        retries += 1;

                        let current_backup = cloned_backup.as_ref().unwrap();

                        active_request = current_backup.try_clone().unwrap();

                        tokio::time::sleep(std::time::Duration::from_millis(500 * retries as u64))
                            .await;

                        continue;
                    }

                    crate::status::note_request_result(false);
                    return Err(RequestError::ReqwestError(err));
                }
            }
        };

        crate::status::note_request_result(true);
        Ok(res)
    }

    #[tracing::instrument(level = "debug", skip(self, request, options, services), fields(dest = %dest.as_ref().display()))]
    pub async fn download_file(
        &self,
        request: impl Into<HttpRequest>,
        dest: impl AsRef<std::path::Path> + Send,
        options: ResponseOptions,
        services: &LauncherServices,
    ) -> Result<(), RequestError> {
        let res = self.send(request).await?;
        let http_stream = res.stream(options, &services.notifier).await?;
        let http_stream = std::pin::pin!(http_stream);

        polyio::write_stream(dest, http_stream).await?;

        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn send_as<T: DeserializeOwned>(&self, request: impl Into<HttpRequest>) -> Result<T, RequestError> {
        let res = self.send(request).await?;
        let status = res.status();
        let url = res.url().to_string();
        let bytes = res.bytes().await?;

        if !status.is_success() {
            return Err(RequestError::HttpStatus {
                status: status.as_u16(),
                url,
                snippet: body_snippet(&bytes),
            });
        }

        serde_json::from_slice(&bytes).map_err(|err| RequestError::DeserializeError {
            source: err,
            type_name: std::any::type_name::<T>().to_string(),
            url,
            status: status.as_u16(),
            snippet: body_snippet(&bytes),
        })
    }

    #[tracing::instrument(level = "debug", skip(self, body, extra_headers), fields(method = %method, %url))]
    pub async fn send_json<T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        url: reqwest::Url,
        body: Option<serde_json::Value>,
        extra_headers: &[(&str, &str)],
    ) -> Result<T, RequestError> {
        let mut request = reqwest::Request::new(method, url);
        for (name, value) in extra_headers {
            request.headers_mut().insert(
                reqwest::header::HeaderName::try_from(*name)?,
                reqwest::header::HeaderValue::try_from(*value)?,
            );
        }

        if let Some(body) = body {
            let bytes = serde_json::to_vec(&body).map_err(RequestError::SerializeError)?;
            request.headers_mut().insert(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            );
            *request.body_mut() = Some(bytes.into());
        }

        self.send_as(request).await
    }
}
