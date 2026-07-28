use oneclient_common::constants;

/// Endpoint and credential settings the transport needs.
///
/// The composition layer pushes these down; the transport never looks anything
/// up. Reaching for the CurseForge key through global state would make the HTTP
/// client depend on the object that owns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetConfig {
	pub curseforge_api_key: String,
	pub modrinth_api_key: Option<String>,
	/// Base for the metadata API (`meta.polyfrost.org`).
	pub metadata_api_url: String,
	/// Base for static assets; never has a trailing slash.
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
	/// Applies user overrides, falling back to the built-in defaults for any
	/// that are absent or blank. Blank is treated as absent so clearing a field
	/// in the settings UI restores the default rather than sending an empty key.
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

	/// Headers Modrinth needs, if a token is configured.
	#[must_use]
	pub fn modrinth_headers(&self) -> Vec<(String, String)> {
		match &self.modrinth_api_key {
			Some(token) => vec![("Authorization".to_string(), token.clone())],
			None => Vec::new(),
		}
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
		let config = NetConfig::default()
			.with_overrides(None, None, None, Some("https://example.test/base/"));

		assert_eq!(config.meta_url_base, "https://example.test/base");
	}

	#[test]
	fn overrides_replace_defaults() {
		let config =
			NetConfig::default().with_overrides(Some("key"), Some("token"), Some("https://a"), None);

		assert_eq!(config.curseforge_api_key, "key");
		assert_eq!(
			config.modrinth_headers(),
			vec![("Authorization".to_string(), "token".to_string())]
		);
		assert_eq!(config.metadata_api_url, "https://a");
	}
}
