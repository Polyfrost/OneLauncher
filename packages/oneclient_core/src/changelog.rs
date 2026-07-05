use std::path::Path;

use reqwest::header;
use reqwest::{Method, StatusCode};

use crate::api_config::meta_url_base;
use crate::paths;
use crate::state::LauncherServices;
use crate::{LauncherError, LauncherResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogGroup {
    pub version: String,
    pub changes: Vec<String>,
}

pub fn parse_changelog(data: &str) -> Vec<ChangelogGroup> {
    let mut groups = Vec::new();

    for line in data.lines() {
        if let Some(version) = line.strip_prefix("# ") {
            groups.push(ChangelogGroup {
                version: version.trim().to_string(),
                changes: Vec::new(),
            });
        } else if let Some(change) = line.strip_prefix("- ") {
            if let Some(group) = groups.last_mut() {
                group.changes.push(change.trim().to_string());
            }
        } else if line.trim() == "###" {
            break;
        }
    }

    groups
}

pub async fn fetch_changelog(services: &LauncherServices) -> LauncherResult<String> {
    let url = format!("{}/oneclient/CHANGE_LOG.md", meta_url_base());
    let cache_path = paths::caches_dir()?.join("CHANGE_LOG.md");
    let etag_path = paths::caches_dir()?.join("CHANGE_LOG.md.etag");

    let stored_etag = read_sidecar_etag(&etag_path).await;
    let url_parsed = url.parse().map_err(LauncherError::UrlError)?;

    let mut request = reqwest::Request::new(Method::GET, url_parsed);
    if let Some(etag) = &stored_etag {
        insert_if_none_match(&mut request, etag);
    }

    match services.requester.send(request).await {
        Ok(res) if res.status() == StatusCode::NOT_MODIFIED => {
            tracing::debug!("changelog cache hit (304)");
            read_cached_changelog(&cache_path).await
        }
        Ok(res) if res.status().is_success() => {
            let server_etag = etag_from_response(&res);
            let bytes = res
                .bytes()
                .await
                .map_err(|err| LauncherError::InvalidSettingsProfile {
                    reason: err.to_string(),
                })?;

            if let Some(parent) = cache_path.parent() {
                polyio::create_dir_all(parent).await?;
            }
            polyio::write(&cache_path, &bytes).await?;
            if let Some(etag) = server_etag {
                let _ = polyio::write(&etag_path, etag.as_bytes()).await;
            }

            String::from_utf8(bytes.to_vec()).map_err(|err| LauncherError::InvalidSettingsProfile {
                reason: err.to_string(),
            })
        }
        Err(err) if cache_path.exists() => {
            tracing::debug!(
                "falling back to cached changelog after remote fetch failed: {err}"
            );
            read_cached_changelog(&cache_path).await
        }
        Err(err) => {
            tracing::error!("failed to fetch changelog from remote: {err}");
            Err(LauncherError::InvalidSettingsProfile {
                reason: err.to_string(),
            })
        }
        Ok(res) => {
            tracing::warn!(
                status = %res.status(),
                "unexpected changelog response; using cache when available"
            );
            if cache_path.exists() {
                read_cached_changelog(&cache_path).await
            } else {
                Err(LauncherError::InvalidSettingsProfile {
                    reason: format!("changelog request failed with HTTP {}", res.status()),
                })
            }
        }
    }
}

async fn read_cached_changelog(path: &Path) -> LauncherResult<String> {
    let bytes = polyio::read(path).await?;
    String::from_utf8(bytes).map_err(|err| LauncherError::InvalidSettingsProfile {
        reason: err.to_string(),
    })
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
    fn parse_changelog_groups_versions_and_bullets() {
        let data = "# 2.0.0\n\n- Faster UI\n- Bug fixes\n# 1.9.0\n- Older change\n###\nignored\n";
        let groups = parse_changelog(data);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].version, "2.0.0");
        assert_eq!(groups[0].changes, vec!["Faster UI", "Bug fixes"]);
        assert_eq!(groups[1].version, "1.9.0");
        assert_eq!(groups[1].changes, vec!["Older change"]);
    }
}
