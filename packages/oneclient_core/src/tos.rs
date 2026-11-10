use serde::{Deserialize, Serialize};

use crate::LauncherResult;
use oneclient_common::constants;

/// Version of the bundled terms. Bump it when the text changes materially so
/// existing installs are asked to accept the new wording.
pub const TERMS_VERSION: u32 = 1;
/// Version of the bundled privacy notice, bumped independently.
pub const PRIVACY_VERSION: u32 = 1;

const TERMS_BODY: &str = include_str!("../assets/legal/terms.md");
const PRIVACY_BODY: &str = include_str!("../assets/legal/privacy.md");

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

#[tracing::instrument(level = "debug")]
pub async fn fetch_terms() -> LauncherResult<TermsDocument> {
    Ok(bundled_terms())
}

/// Terms and privacy notice shipped with the binary.
///
/// They used to be downloaded from Polyfrost's metadata service, which showed
/// upstream's legal text as if it were ours. A fork must present its own notice,
/// and it has to work with no network at all, so the text is compiled in.
#[must_use]
pub fn bundled_terms() -> TermsDocument {
    TermsDocument {
        version: TERMS_VERSION,
        privacy_version: Some(PRIVACY_VERSION),
        updated_at: None,
        terms: TERMS_BODY.to_string(),
        privacy: Some(PRIVACY_BODY.to_string()),
        terms_url: Some(constants::TOS_URL.to_string()),
        privacy_url: Some(constants::PRIVACY_URL.to_string()),
    }
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

    #[test]
    fn bundled_terms_are_ours_and_complete() {
        let document = bundled_terms();

        assert_eq!(document.version, TERMS_VERSION);
        assert_eq!(document.privacy_version(), PRIVACY_VERSION);
        assert!(document.terms.contains("RemixClient"));
        assert!(!document.terms.contains("polyfrost.org/legal"));
        assert!(document.privacy_body().is_some_and(|body| body.len() > 200));
    }
}
