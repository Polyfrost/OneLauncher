
use serde::{Deserialize, Serialize};

use oneclient_common::constants;
use oneclient_common::paths;
use oneclient_net::{EtagPolicy, fetch_cached};
use oneclient_net::RequestClient;
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

#[tracing::instrument(level = "debug", skip(net))]
pub async fn fetch_terms(net: &RequestClient) -> LauncherResult<TermsDocument> {
    let url = format!(
        "{}/oneclient/tos.json",
        net.config().meta_url_base
    );
    let cache_path = paths::caches_dir()?.join("TERMS.json");

    let fetched = fetch_cached(net, &url, &cache_path, EtagPolicy::CommitNow)
        .await?
        .ok_or_else(|| LauncherError::InvalidSettingsProfile {
            reason: "terms are unavailable and not cached".to_string(),
        })?;

    Ok(fetched.json()?)
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
