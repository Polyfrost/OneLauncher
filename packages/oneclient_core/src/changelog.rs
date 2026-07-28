

use oneclient_common::paths;
use oneclient_net::{EtagPolicy, fetch_cached};
use oneclient_net::RequestClient;
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

#[tracing::instrument(level = "debug", skip(net))]
pub async fn fetch_changelog(net: &RequestClient) -> LauncherResult<String> {
    let url = format!(
        "{}/oneclient/CHANGE_LOG.md",
        net.config().meta_url_base
    );
    let cache_path = paths::caches_dir()?.join("CHANGE_LOG.md");

    let fetched = fetch_cached(net, &url, &cache_path, EtagPolicy::CommitNow)
        .await?
        .ok_or_else(|| LauncherError::InvalidSettingsProfile {
            reason: "changelog is unavailable and not cached".to_string(),
        })?;

    Ok(fetched.text())
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
