use std::sync::{Arc, RwLock};

use regex::Regex;
use serde::Deserialize;

use oneclient_common::constants::CRASH_DATA_URL;

const BUNDLED: &str = include_str!("../../assets/crashes.json");

const CACHE_FILE: &str = "crashes.json";

const MIN_CAUSE_LEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Contains,
    Regex,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Cause {
    pub method: Method,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Fix {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub fixtype: Option<usize>,
    pub fix: String,
    #[serde(default)]
    pub causes: Vec<Cause>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FixType {
    pub name: String,
    #[serde(default)]
    pub no_ingame_display: bool,
    #[serde(default)]
    pub server_crashes: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrashData {
    #[serde(default)]
    pub fixes: Vec<Fix>,
    #[serde(default)]
    pub fixtypes: Vec<FixType>,
    #[serde(default)]
    pub default_fix_type: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixMatch {
    pub text: String,
    pub kind: String,
    pub name: Option<String>,
    pub specificity: usize,
    informational: bool,
}

impl CrashData {
    pub fn parse(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }

    #[must_use]
    pub fn bundled() -> Self {
        Self::parse(BUNDLED).unwrap_or_else(|err| {
            tracing::error!("bundled crash data is not valid json: {err}");
            Self {
                fixes: Vec::new(),
                fixtypes: Vec::new(),
                default_fix_type: 0,
            }
        })
    }

    fn fixtype_of(&self, fix: &Fix) -> Option<&FixType> {
        self.fixtypes
            .get(fix.fixtype.unwrap_or(self.default_fix_type))
    }

    #[must_use]
    pub fn matches(&self, document: &str) -> Vec<FixMatch> {
        let mut found: Vec<FixMatch> = self
            .fixes
            .iter()
            .filter(|fix| !fix.causes.is_empty())
            .filter_map(|fix| {
                let kind = self.fixtype_of(fix);

                if kind.is_some_and(|k| k.server_crashes) {
                    return None;
                }

                if !fix.causes.iter().all(|cause| matches_cause(cause, document)) {
                    return None;
                }

                Some(FixMatch {
                    text: fix.fix.clone(),
                    kind: kind.map_or_else(|| "Solution".to_string(), |k| k.name.clone()),
                    name: fix.name.clone(),
                    specificity: fix.causes.len(),
                    informational: kind.is_some_and(|k| k.no_ingame_display),
                })
            })
            .collect();

        found.sort_by(|a, b| {
            a.informational
                .cmp(&b.informational)
                .then_with(|| rank(&b.kind).cmp(&rank(&a.kind)))
                .then_with(|| b.specificity.cmp(&a.specificity))
        });
        found
    }
}

fn rank(kind: &str) -> u8 {
    match kind {
        "Solution" => 2,
        "Recommendations" => 1,
        _ => 0,
    }
}

fn matches_cause(cause: &Cause, document: &str) -> bool {
    if cause.value.len() < MIN_CAUSE_LEN {
        return false;
    }

    match cause.method {
        Method::Contains => document.contains(&cause.value),
        Method::Regex => match Regex::new(&cause.value) {
            Ok(re) => re.is_match(document),
            Err(err) => {
                tracing::warn!(pattern = %cause.value, "crash data regex does not compile: {err}");
                false
            }
        },
    }
}

static STORE: RwLock<Option<Arc<CrashData>>> = RwLock::new(None);

#[must_use]
pub fn current() -> Arc<CrashData> {
    if let Some(data) = STORE.read().ok().and_then(|guard| guard.clone()) {
        return data;
    }

    let bundled = Arc::new(CrashData::bundled());
    if let Ok(mut guard) = STORE.write() {
        guard.get_or_insert_with(|| Arc::clone(&bundled));
    }
    bundled
}

fn store(data: CrashData) {
    if let Ok(mut guard) = STORE.write() {
        *guard = Some(Arc::new(data));
    }
}

fn cache_path() -> Option<std::path::PathBuf> {
    oneclient_common::paths::caches_dir()
        .ok()
        .map(|dir| dir.join(CACHE_FILE))
}

pub async fn load(requester: &oneclient_net::RequestClient) {
    if let Some(path) = cache_path()
        && let Ok(raw) = polyio::read_to_string(&path).await
    {
        match CrashData::parse(&raw) {
            Ok(data) => {
                tracing::debug!(fixes = data.fixes.len(), "loaded cached crash data");
                store(data);
            }
            Err(err) => tracing::warn!("cached crash data is unreadable: {err}"),
        }
    }

    refresh(requester).await;
}

async fn refresh(requester: &oneclient_net::RequestClient) {
    let raw = match requester.http().get(CRASH_DATA_URL).send().await {
        Ok(res) if res.status().is_success() => match res.text().await {
            Ok(raw) => raw,
            Err(err) => {
                tracing::warn!("could not read the crash data response: {err}");
                return;
            }
        },
        Ok(res) => {
            tracing::warn!(status = %res.status(), "crash data refresh rejected");
            return;
        }
        Err(err) => {
            tracing::warn!("crash data refresh failed: {err}");
            return;
        }
    };

    let data = match CrashData::parse(&raw) {
        Ok(data) => data,
        Err(err) => {
            tracing::warn!("downloaded crash data is not valid json: {err}");
            return;
        }
    };

    if data.fixes.is_empty() {
        tracing::warn!("downloaded crash data has no fixes, keeping the current set");
        return;
    }

    tracing::info!(fixes = data.fixes.len(), "refreshed crash data");

    if let Some(path) = cache_path() {
        let written = match path.parent() {
            Some(dir) => polyio::create_dir_all(dir)
                .await
                .and(polyio::write(&path, raw.as_bytes()).await),
            None => polyio::write(&path, raw.as_bytes()).await,
        };

        if let Err(err) = written {
            tracing::warn!("could not cache crash data: {err}");
        }
    }

    store(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> CrashData {
        CrashData::parse(
            r#"{
                "fixes": [
                    {
                        "name": "two",
                        "fixtype": 1,
                        "fix": "Remove the mod",
                        "causes": [
                            {"method": "contains", "value": "NullPointerException"},
                            {"method": "contains", "value": "bingobrewers"}
                        ]
                    },
                    {
                        "fixtype": 2,
                        "fix": "Try updating",
                        "causes": [{"method": "contains", "value": "NullPointerException"}]
                    },
                    {
                        "fixtype": 0,
                        "fix": "Old forge",
                        "causes": [{"method": "contains", "value": "NullPointerException"}]
                    },
                    {
                        "fixtype": 3,
                        "fix": "Server side",
                        "causes": [{"method": "contains", "value": "NullPointerException"}]
                    }
                ],
                "fixtypes": [
                    {"name": "Info", "no_ingame_display": true},
                    {"name": "Solution"},
                    {"name": "Recommendations"},
                    {"name": "Disconnect Solution", "server_crashes": true}
                ],
                "default_fix_type": 2
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn the_bundled_snapshot_parses() {
        let data = CrashData::bundled();
        assert!(data.fixes.len() > 100, "{} fixes", data.fixes.len());
        assert_eq!(data.fixtypes.len(), 4);
    }

    #[test]
    fn every_cause_has_to_match() {
        let data = data();

        let hits = data.matches("java.lang.NullPointerException: Initializing game");
        assert!(
            !hits.iter().any(|hit| hit.name.as_deref() == Some("two")),
            "a rule matched with only one of its two causes"
        );

        let both = data.matches("java.lang.NullPointerException at bingobrewers.Mod");
        assert_eq!(both.first().unwrap().name.as_deref(), Some("two"));
    }

    #[test]
    fn causes_may_sit_on_different_lines() {
        let document = "java.lang.NullPointerException: Initializing game\n\
                        \tat net.minecraft.client.Minecraft.run\n\
                        \tat bingobrewers.Overlay.render";

        let hits = data().matches(document);
        assert_eq!(hits.first().unwrap().name.as_deref(), Some("two"));
    }

    #[test]
    fn a_disconnect_rule_never_matches_a_crash() {
        let hits = data().matches("java.lang.NullPointerException");
        assert!(
            hits.iter().all(|hit| hit.text != "Server side"),
            "a server_crashes rule leaked into a crash match"
        );
    }

    #[test]
    fn solutions_outrank_recommendations_and_info_sinks() {
        let hits = data().matches("java.lang.NullPointerException at bingobrewers.Mod");

        let order: Vec<&str> = hits.iter().map(|hit| hit.text.as_str()).collect();
        assert_eq!(order, vec!["Remove the mod", "Try updating", "Old forge"]);
    }

    #[test]
    fn an_ordinary_session_matches_nothing() {
        let hits = data().matches("[Render thread/INFO]: Setting user: Dev\n[main/INFO]: Stopping!");
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn a_broken_regex_does_not_match_everything() {
        let data = CrashData::parse(
            r#"{"fixes":[{"fix":"x","causes":[{"method":"regex","value":"("}]}],
                "fixtypes":[{"name":"Solution"}],"default_fix_type":0}"#,
        )
        .unwrap();

        assert!(data.matches("anything at all").is_empty());
    }
}
