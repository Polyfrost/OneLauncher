use oneclient_common::constants;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetConfig {
    pub curseforge_api_key: String,
    pub modrinth_api_key: Option<String>,
    pub metadata_api_url: String,
    /// Never has a trailing slash
    pub meta_url_base: String,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            curseforge_api_key: constants::CURSEFORGE_API_KEY.to_string(),
            modrinth_api_key: None,
            metadata_api_url: constants::METADATA_API_URL.to_string(),
            meta_url_base: constants::META_URL_BASE.to_string(),
        }
    }
}

impl NetConfig {
    /// Blank is treated as absent so clearing a field in the settings UI
    /// restores the default rather than sending an empty key
    #[must_use]
    pub fn with_overrides(
        mut self,
        curseforge_api_key: Option<&str>,
        modrinth_api_key: Option<&str>,
        metadata_api_url: Option<&str>,
        meta_url_base: Option<&str>,
    ) -> Self {
        if let Some(value) = non_empty(curseforge_api_key) {
            self.curseforge_api_key = value;
        }
        self.modrinth_api_key = non_empty(modrinth_api_key);
        if let Some(value) = non_empty(metadata_api_url) {
            self.metadata_api_url = value;
        }
        if let Some(value) = non_empty(meta_url_base) {
            self.meta_url_base = value;
        }
        self.meta_url_base = self.meta_url_base.trim_end_matches('/').to_string();
        self
    }

	#[must_use]
	pub fn modrinth_headers(&self) -> Vec<(String, String)> {
		match &self.modrinth_api_key {
			Some(token) => vec![("Authorization".to_string(), token.clone())],
			None => Vec::new(),
		}
	}

	/// Whether `url` targets a host the user explicitly configured as the
	/// custom API endpoint or meta URL base.
	#[must_use]
	pub fn allows_host(&self, url: &reqwest::Url) -> bool {
		let Some(host) = url.host_str() else {
			return false;
		};
		let port = url.port_or_known_default();

		[self.metadata_api_url.as_str(), self.meta_url_base.as_str()]
			.into_iter()
			.filter_map(|base| base.parse::<reqwest::Url>().ok())
			.any(|base| {
				base.host_str() == Some(host) && base.port_or_known_default() == port
			})
	}
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_overrides_fall_back_to_defaults() {
        let config = NetConfig::default().with_overrides(Some("   "), Some(""), None, None);

        assert_eq!(config.curseforge_api_key, constants::CURSEFORGE_API_KEY);
        assert_eq!(config.modrinth_api_key, None);
        assert_eq!(config.metadata_api_url, constants::METADATA_API_URL);
    }

    #[test]
    fn meta_url_base_never_keeps_a_trailing_slash() {
        let config = NetConfig::default().with_overrides(
            None,
            None,
            None,
            Some("https://example.test/base/"),
        );

        assert_eq!(config.meta_url_base, "https://example.test/base");
    }

    #[test]
    fn overrides_replace_defaults() {
        let config = NetConfig::default().with_overrides(
            Some("key"),
            Some("token"),
            Some("https://a"),
            None,
        );

        assert_eq!(config.curseforge_api_key, "key"); 
        assert_eq!(
          config.modrinth_headers(),
          vec![("Authorization".to_string(), "token".to_string())]
        );
        assert_eq!(config.metadata_api_url, "https://a");
	  }

    fn parsed(url: &str) -> reqwest::Url {
        url.parse().unwrap()
    }

    #[test]
    fn allows_host_matches_a_configured_local_endpoint() {
        let config = NetConfig::default().with_overrides(None, None, None, Some("http://localhost:8000"));

        assert!(config.allows_host(&parsed("http://localhost:8000/icon.png")));
        assert!(config.allows_host(&parsed("http://localhost:8000/")));
    }

    #[test]
    fn allows_host_requires_the_same_port() {
        let config = NetConfig::default().with_overrides(None, None, None, Some("http://localhost:8000"));

        assert!(
            !config.allows_host(&parsed("http://localhost:9090/icon.png")),
            "a different port must not be treated as the configured endpoint"
        );
    }

    #[test]
    fn allows_host_ignores_hosts_that_were_not_configured() {
        let config = NetConfig::default().with_overrides(None, None, Some("http://localhost:8000"), None);

        assert!(config.allows_host(&parsed("http://localhost:8000/icon.png")));
        assert!(!config.allows_host(&parsed("http://127.0.0.1:8000/icon.png")));
        assert!(!config.allows_host(&parsed("http://example.com/icon.png")));
    }

    #[test]
    fn allows_host_is_false_when_no_custom_endpoint_is_set() {
        let config = NetConfig::default();

        assert!(!config.allows_host(&parsed("http://localhost:8000/icon.png")));
    }
}
