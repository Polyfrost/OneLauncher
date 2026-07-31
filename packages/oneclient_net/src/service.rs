use std::sync::Arc;

use arc_swap::ArcSwap;
use oneclient_events::EventBus;
use reqwest::{ClientBuilder, Response};
use serde::de::DeserializeOwned;
use tokio::sync::Semaphore;

use crate::config::NetConfig;
use crate::error::{RequestError, body_snippet};
use crate::request::HttpRequest;
use crate::response::{ResponseExt, ResponseOptions};

const MAX_THROTTLE_RETRIES: u32 = 6;

/// Ceiling on requests waiting for response headers. This is a backstop against
/// a runaway fan-out, not the download throttle. Per-phase concurrency is set
/// by the callers, and this has to stay above their sum or it becomes the
/// bottleneck instead. Permits are released once headers arrive, so streaming
/// bodies don't hold a slot.
const MAX_INFLIGHT_REQUESTS: usize = 64;

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

fn is_curseforge_host(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" {
        return false;
    }

    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();

    matches!(host.as_str(), "curseforge.com" | "forgecdn.net")
        || host.ends_with(".curseforge.com")
        || host.ends_with(".forgecdn.net")
}

fn apply_curseforge_auth(
    request: &mut reqwest::Request,
    api_key: &str,
) -> Result<(), RequestError> {
    const API_KEY_HEADER: reqwest::header::HeaderName =
        reqwest::header::HeaderName::from_static("x-api-key");

    if !is_curseforge_host(request.url()) {
        return Ok(());
    }

    let headers = request.headers_mut();
    if headers.contains_key(&API_KEY_HEADER) {
        return Ok(());
    }

    let mut value = reqwest::header::HeaderValue::try_from(api_key)?;
    value.set_sensitive(true);
    headers.insert(API_KEY_HEADER, value);

    Ok(())
}

/// Adds the bundled Mozilla roots as a fallback behind the Windows trust store.
///
/// This does not replace the system store. `rustls-platform-verifier` builds
/// the chain against Windows first and only retries with these roots when that
/// came back untrusted or partial, so an intercepting proxy or an
/// enterprise-installed CA still works exactly as before.
///
/// It matters because Windows populates its root store lazily from Windows
/// Update. On a machine where root auto-update is disabled by policy — routine
/// on managed school and workplace images — or on a trimmed-down install, a
/// root that the rest of the world has can simply be absent. Every other
/// client those users try carries its own roots (Chrome since 105, Firefox via
/// NSS, Java-based launchers via `cacerts`), so sign-in works everywhere
/// except here. macOS and Linux do not have this failure mode, and adding
/// roots there would be additive rather than a fallback, so this is Windows
/// only.
#[cfg(target_os = "windows")]
fn add_fallback_roots(mut builder: ClientBuilder) -> ClientBuilder {
    for der in webpki_root_certs::TLS_SERVER_ROOT_CERTS {
        match reqwest::Certificate::from_der(der) {
            Ok(root) => builder = builder.add_root_certificate(root),
            // A malformed entry would be a bug in the bundle, not something the
            // user can act on. Dropping one root beats failing to build a client.
            Err(err) => tracing::warn!("skipping malformed fallback root: {err}"),
        }
    }

    builder
}

#[derive(Clone)]
pub struct RequestClient {
    client: reqwest::Client,
    semaphore: Arc<Semaphore>,
    /// Shared across every clone, so a settings save is visible to in-flight
    /// handles. `ArcSwap` rather than a lock because this is read once per
    /// outbound request (with 64 in flight a lock would serialise them) and
    /// written only when the user saves settings.
    config: Arc<ArcSwap<NetConfig>>,
}

impl RequestClient {
    pub fn http(&self) -> &reqwest::Client {
        &self.client
    }

    /// The endpoint/credential config every request is sent with.
    #[must_use]
    pub fn config(&self) -> arc_swap::Guard<Arc<NetConfig>> {
        self.config.load()
    }

    /// Replaces the config for this client and every clone of it.
    pub fn set_config(&self, config: NetConfig) {
        self.config.store(Arc::new(config));
    }

    pub fn new(config: NetConfig) -> Result<Self, RequestError> {
        let mut builder = ClientBuilder::new()
            .connect_timeout(std::time::Duration::from_secs(10))
            .read_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_mins(10))
            .tls_backend_rustls()
            .user_agent(format!(
                "OneClient {} ({})",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_HOMEPAGE")
            ));

        #[cfg(not(target_os = "windows"))]
        {
            builder = builder.hickory_dns(true);
        }

        #[cfg(target_os = "windows")]
        {
            builder = add_fallback_roots(builder);
        }

        let client = builder.build()?;

        Ok(Self {
            client,
            semaphore: Arc::new(Semaphore::new(MAX_INFLIGHT_REQUESTS)),
            config: Arc::new(ArcSwap::from_pointee(config)),
        })
    }
}

impl RequestClient {
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn send(&self, request: impl Into<HttpRequest>) -> Result<Response, RequestError> {
        let mut request: HttpRequest = request.into();
        let mut retries = 0;
        let mut throttle_retries = 0u32;

        apply_curseforge_auth(&mut request.request, &self.config.load().curseforge_api_key)?;

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

    #[tracing::instrument(level = "debug", skip(self, request, options, events), fields(dest = %dest.as_ref().display()))]
    pub async fn download_file(
        &self,
        request: impl Into<HttpRequest>,
        dest: impl AsRef<std::path::Path> + Send,
        options: ResponseOptions,
        events: &EventBus,
    ) -> Result<(), RequestError> {
        let res = self.send(request).await?;
        let size_hint = res.content_length();
        let http_stream = res.stream(options, events).await?;
        let http_stream = std::pin::pin!(http_stream);

        polyio::write_stream(dest, http_stream, size_hint).await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn request(url: &str) -> reqwest::Request {
        reqwest::Request::new(reqwest::Method::GET, url.parse().unwrap())
    }

    fn api_key_of(url: &str) -> Option<String> {
        let mut req = request(url);
        apply_curseforge_auth(&mut req, oneclient_common::constants::CURSEFORGE_API_KEY).unwrap();
        req.headers()
            .get("x-api-key")
            .map(|v| v.to_str().unwrap().to_string())
    }

    #[test]
    fn attaches_key_to_curseforge_api_and_cdn() {
        for url in [
            "https://api.curseforge.com/v1/mods/search",
            "https://media.forgecdn.net/avatars/1/2/icon.png",
            "https://edge.forgecdn.net/files/1/2/mod.jar",
            "https://www.curseforge.com/minecraft",
            "https://forgecdn.net/thing",
        ] {
            assert_eq!(
                api_key_of(url).as_deref(),
                Some(oneclient_common::constants::CURSEFORGE_API_KEY),
                "expected key on {url}"
            );
        }
    }

    #[test]
    fn ignores_unrelated_and_insecure_hosts() {
        for url in [
            "https://api.modrinth.com/v2/search",
            "https://cdn.modrinth.com/data/x/y.jar",
            "https://notcurseforge.com/x",
            "https://evil-forgecdn.net.attacker.com/x",
            "http://media.forgecdn.net/insecure.png",
        ] {
            assert_eq!(api_key_of(url), None, "unexpected key on {url}");
        }
    }

    #[test]
    fn does_not_override_explicit_key() {
        let mut req = request("https://api.curseforge.com/v1/mods");
        req.headers_mut()
            .insert("x-api-key", reqwest::header::HeaderValue::from_static("mine"));
        apply_curseforge_auth(&mut req, oneclient_common::constants::CURSEFORGE_API_KEY).unwrap();
        assert_eq!(req.headers()["x-api-key"], "mine");
    }
}
