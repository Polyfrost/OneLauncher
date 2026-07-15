use std::path::Path;

use reqwest::header;
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};

use crate::api_config::meta_url_base;
use crate::constants;
use crate::paths;
use crate::state::LauncherServices;
use crate::{LauncherError, LauncherResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermsDocument {
    pub version: u32,
    #[serde(default)]
    pub privacy_version: Option<u32>,
    #[serde(default)]
    pub updated_at: Option<String>,
    pub terms: String,
    #[serde(default)]
    pub privacy: Option<String>,
    #[serde(default)]
    pub terms_url: Option<String>,
    #[serde(default)]
    pub privacy_url: Option<String>,
}

impl TermsDocument {
    pub fn terms_url(&self) -> &str {
        self.terms_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .unwrap_or(constants::TOS_URL)
    }

    pub fn privacy_url(&self) -> &str {
        self.privacy_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .unwrap_or(constants::PRIVACY_URL)
    }

    pub fn privacy_version(&self) -> u32 {
        self.privacy_version.unwrap_or(self.version)
    }

    pub fn privacy_body(&self) -> Option<&str> {
        self.privacy
            .as_deref()
            .filter(|body| !body.trim().is_empty())
    }
}

#[tracing::instrument(level = "debug", skip(services))]
pub async fn fetch_terms(services: &LauncherServices) -> LauncherResult<TermsDocument> {
    let url = format!("{}/oneclient/tos.json", meta_url_base());
    let cache_path = paths::caches_dir()?.join("TERMS.json");
    let etag_path = paths::caches_dir()?.join("TERMS.json.etag");

    let stored_etag = read_sidecar_etag(&etag_path).await;
    let url_parsed = url.parse().map_err(LauncherError::UrlError)?;

    let mut request = reqwest::Request::new(Method::GET, url_parsed);
    if let Some(etag) = &stored_etag {
        insert_if_none_match(&mut request, etag);
    }

    match services.requester.send(request).await {
        Ok(res) if res.status() == StatusCode::NOT_MODIFIED => {
            tracing::debug!("terms cache hit (304)");
            read_cached_terms(&cache_path).await
        }
        Ok(res) if res.status().is_success() => {
            let server_etag = etag_from_response(&res);
            let bytes = res
                .bytes()
                .await
                .map_err(|err| LauncherError::InvalidSettingsProfile {
                    reason: err.to_string(),
                })?;

            let document: TermsDocument = serde_json::from_slice(&bytes)?;

            if let Some(parent) = cache_path.parent() {
                polyio::create_dir_all(parent).await?;
            }
            polyio::write(&cache_path, &bytes).await?;
            if let Some(etag) = server_etag {
                let _ = polyio::write(&etag_path, etag.as_bytes()).await;
            }

            Ok(document)
        }
        Err(err) if cache_path.exists() => {
            tracing::debug!("falling back to cached terms after remote fetch failed: {err}");
            read_cached_terms(&cache_path).await
        }
        Err(err) => {
            tracing::error!("failed to fetch terms from remote: {err}");
            Err(LauncherError::InvalidSettingsProfile {
                reason: err.to_string(),
            })
        }
        Ok(res) => {
            tracing::warn!(
                status = %res.status(),
                "unexpected terms response; using cache when available"
            );
            if cache_path.exists() {
                read_cached_terms(&cache_path).await
            } else {
                Err(LauncherError::InvalidSettingsProfile {
                    reason: format!("terms request failed with HTTP {}", res.status()),
                })
            }
        }
    }
}

async fn read_cached_terms(path: &Path) -> LauncherResult<TermsDocument> {
    let bytes = polyio::read(path).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn insert_if_none_match(request: &mut reqwest::Request, etag: &str) {
    if let Ok(value) = header::HeaderValue::from_str(etag) {
        request.headers_mut().insert(header::IF_NONE_MATCH, value);
    }
}

fn etag_from_response(res: &reqwest::Response) -> Option<String> {
    res.headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

async fn read_sidecar_etag(path: &Path) -> Option<String> {
    polyio::read(path)
        .await
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .filter(|etag| !etag.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_document() {
        let raw = br###"{"version":2,"terms":"## Terms\n\nBe nice."}"###;
        let document: TermsDocument = serde_json::from_slice(raw).unwrap();

        assert_eq!(document.version, 2);
        assert_eq!(document.terms, "## Terms\n\nBe nice.");
        assert_eq!(document.terms_url(), constants::TOS_URL);
        assert_eq!(document.privacy_url(), constants::PRIVACY_URL);
    }

    #[test]
    fn privacy_version_falls_back_to_terms_version() {
        let raw = br#"{"version":4,"terms":"x"}"#;
        let document: TermsDocument = serde_json::from_slice(raw).unwrap();

        assert_eq!(document.privacy_version(), 4);
        assert_eq!(document.privacy_body(), None);
    }

    #[test]
    fn privacy_version_is_independent_when_published() {
        let raw = br###"{"version":4,"privacy_version":2,"terms":"x","privacy":"## Privacy"}"###;
        let document: TermsDocument = serde_json::from_slice(raw).unwrap();

        assert_eq!(document.version, 4);
        assert_eq!(document.privacy_version(), 2);
        assert_eq!(document.privacy_body(), Some("## Privacy"));
    }

    #[test]
    fn blank_privacy_body_is_treated_as_absent() {
        let raw = br#"{"version":1,"terms":"x","privacy":"   "}"#;
        let document: TermsDocument = serde_json::from_slice(raw).unwrap();

        assert_eq!(document.privacy_body(), None);
    }

    #[test]
    fn document_urls_override_constants() {
        let raw = br#"{"version":1,"terms":"x","terms_url":"https://example.com/tos","privacy_url":"  "}"#;
        let document: TermsDocument = serde_json::from_slice(raw).unwrap();

        assert_eq!(document.terms_url(), "https://example.com/tos");
        assert_eq!(document.privacy_url(), constants::PRIVACY_URL);
    }
}
