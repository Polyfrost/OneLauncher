use std::borrow::Cow;
use std::sync::{Arc, RwLock};

use regex::Regex;
use serde::Deserialize;

use oneclient_common::constants::CRASH_DATA_URL;

const BUNDLED: &str = include_str!("../../assets/crashes.json");

const CACHE_FILE: &str = "crashes.json";

const MIN_CAUSE_LEN: usize = 4;

const DOWNLOAD_CAP: usize = 4 * 1024 * 1024;

const MODULE_LIST_MARKERS: [&str; 2] = ["Dynamic libraries:", "Loaded modules:"];
const MIN_ADDRESS_DIGITS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(from = "String")]
pub enum Method {
    Contains,
    ContainsNot,
    Regex,
}

impl From<String> for Method {
    fn from(raw: String) -> Self {
        match raw.to_ascii_lowercase().as_str() {
            "contains_not" | "containsnot" => Self::ContainsNot,
            "regex" => Self::Regex,
            _ => Self::Contains,
        }
    }
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

                if kind.is_some_and(|k| k.server_crashes || k.no_ingame_display) {
                    return None;
                }

                if !fix.causes.iter().all(|cause| matches_cause(cause, document)) {
                    return None;
                }

                Some(FixMatch {
                    text: fix.fix.clone(),
                    kind: kind.map_or_else(|| SOLUTION_KIND.to_string(), |k| k.name.clone()),
                    name: fix.name.clone(),
                    specificity: fix.causes.len(),
                })
            })
            .collect();

        found.sort_by(|a, b| {
            rank(&b.kind)
                .cmp(&rank(&a.kind))
                .then_with(|| b.specificity.cmp(&a.specificity))
        });
        found
    }
}

const SOLUTION_KIND: &str = "Solution";
const RECOMMENDATION_KIND: &str = "Recommendations";

fn rank(kind: &str) -> u8 {
    match kind {
        SOLUTION_KIND => 2,
        RECOMMENDATION_KIND => 1,
        _ => 0,
    }
}

fn matches_cause(cause: &Cause, document: &str) -> bool {
    if cause.value.len() < MIN_CAUSE_LEN {
        return false;
    }

    match cause.method {
        Method::Contains => document.contains(&cause.value),
        Method::ContainsNot => !document.contains(&cause.value),
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

#[must_use]
pub fn has_solution(fixes: &[oneclient_events::CrashFix]) -> bool {
    fixes.iter().any(|fix| fix.kind == SOLUTION_KIND)
}

fn is_module_entry(line: &str) -> bool {
    let Some((address, _)) = line.trim_start().split_once([' ', '-', '\t']) else {
        return false;
    };

    let address = address.trim_start_matches("0x");

    address.len() >= MIN_ADDRESS_DIGITS && address.chars().all(|c| c.is_ascii_hexdigit())
}

fn without_module_list(document: &str) -> Cow<'_, str> {
    let names_modules = |line: &str| MODULE_LIST_MARKERS.iter().any(|m| line.contains(m));

    if !names_modules(document) {
        return Cow::Borrowed(document);
    }

    let mut kept = String::with_capacity(document.len());
    let mut skipping = false;

    for line in document.lines() {
        if skipping {
            if is_module_entry(line) {
                continue;
            }
            skipping = false;
        }

        if names_modules(line) {
            skipping = true;
            continue;
        }

        kept.push_str(line);
        kept.push('\n');
    }

    Cow::Owned(kept)
}

#[must_use]
pub fn fixes_for(document: &str) -> Vec<oneclient_events::CrashFix> {
    let document = without_module_list(document);

    current()
        .matches(&document)
        .into_iter()
        .map(|hit| oneclient_events::CrashFix {
            text: hit.text,
            kind: hit.kind,
        })
        .collect()
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
    let Some(path) = cache_path() else {
        tracing::warn!("no cache directory for crash data, keeping the bundled set");
        return;
    };

    let fetched = match oneclient_net::fetch_cached(
        requester,
        CRASH_DATA_URL,
        &path,
        oneclient_net::EtagPolicy::Defer,
    )
    .await
    {
        Ok(Some(fetched)) => fetched,
        Ok(None) => {
            tracing::debug!("no crash data on disk or from the network, keeping the bundled set");
            return;
        }
        Err(err) => {
            tracing::warn!("crash data refresh failed: {err}");
            return;
        }
    };

    if fetched.bytes.len() > DOWNLOAD_CAP {
        tracing::warn!(
            bytes = fetched.bytes.len(),
            "crash data is over the size cap, keeping the current set"
        );
        return;
    }

    let data = match CrashData::parse(&fetched.text()) {
        Ok(data) => data,
        Err(err) => {
            tracing::warn!("crash data is not valid json: {err}");
            return;
        }
    };

    if data.fixes.is_empty() {
        tracing::warn!("crash data has no fixes, keeping the current set");
        return;
    }

    tracing::info!(
        fixes = data.fixes.len(),
        changed = fetched.changed,
        "loaded crash data"
    );

    if let Some(etag) = &fetched.etag {
        oneclient_net::commit_etag(&path, etag).await;
    }

    store(data);
}

#[cfg(test)]
mod tests {
    use super::*;

    const NATIVE_DUMP: &str = "\
#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x00007ffb1e2d1e40
# Problematic frame:
# C  [atio6axx.dll+0x9d1e40]

Dynamic libraries:
0x00007ff7b2e50000 - 0x00007ff7b2e60000 \tC:\\Program Files\\Java\\bin\\javaw.exe
0x00007ffb1e200000 - 0x00007ffb1ed40000 \tC:\\Windows\\System32\\atio6axx.dll
0x00007ffb0a100000 - 0x00007ffb0a1c0000 \tC:\\Users\\me\\AppData\\Medal\\medal-hook64.dll

VM Arguments:
java_command: net.minecraft.client.main.Main
";

    #[test]
    fn the_module_list_is_out_of_scope_for_matching() {
        let kept = without_module_list(NATIVE_DUMP);

        assert!(!kept.contains("medal-hook64.dll"));
        assert!(!kept.contains("javaw.exe"));

        assert!(kept.contains("EXCEPTION_ACCESS_VIOLATION"));
        assert!(kept.contains("[atio6axx.dll+0x9d1e40]"));
        assert!(kept.contains("VM Arguments:"));
        assert!(kept.contains("java_command: net.minecraft.client.main.Main"));
    }

    #[test]
    fn an_ordinary_log_is_left_alone() {
        let log = "[12:04:11] [main/INFO]: Loading 42 mods\n\
                   java.lang.NullPointerException: Initializing game\n\
                   \tat net.minecraft.client.Minecraft.run\n";

        assert!(matches!(without_module_list(log), Cow::Borrowed(_)));
        assert_eq!(without_module_list(log), log);
    }

    #[test]
    fn only_address_rows_are_treated_as_modules() {
        assert!(is_module_entry(
            "0x00007ff7b2e50000 - 0x00007ff7b2e60000 \tC:\\javaw.exe"
        ));
        assert!(is_module_entry(
            "7f8a1c000000-7f8a1c021000 r-xp 00000000 08:01 1234 /usr/lib/libc.so"
        ));

        assert!(!is_module_entry("[12:04:11] [main/INFO]: Loading 42 mods"));
        assert!(!is_module_entry("\tat net.minecraft.client.Minecraft.run"));
        assert!(!is_module_entry("VM Arguments:"));
        assert!(!is_module_entry(""));
    }

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
    fn solutions_outrank_recommendations_and_info_never_shows() {
        let hits = data().matches("java.lang.NullPointerException at bingobrewers.Mod");

        let order: Vec<&str> = hits.iter().map(|hit| hit.text.as_str()).collect();
        assert_eq!(order, vec!["Remove the mod", "Try updating"]);
    }

    #[test]
    fn an_unknown_match_method_falls_back_to_contains() {
        let data = CrashData::parse(
            r#"{"fixes":[{"fix":"x","causes":[{"method":"startswith","value":"NullPointer"}]}],
                "fixtypes":[{"name":"Solution"}],"default_fix_type":0}"#,
        )
        .expect("an unfamiliar method must not fail the whole file");

        assert_eq!(data.matches("java.lang.NullPointerException").len(), 1);
    }

    #[test]
    fn a_contains_not_cause_narrows_a_rule() {
        let data = CrashData::parse(
            r#"{"fixes":[{"fix":"x","causes":[
                    {"method":"contains","value":"NullPointerException"},
                    {"method":"contains_not","value":"OptiFine"}
                ]}],
                "fixtypes":[{"name":"Solution"}],"default_fix_type":0}"#,
        )
        .unwrap();

        assert_eq!(data.matches("java.lang.NullPointerException").len(), 1);
        assert!(
            data.matches("java.lang.NullPointerException with OptiFine")
                .is_empty()
        );
    }

    #[test]
    fn recommendations_alone_are_not_worth_a_dialog() {
        let fixes = fixes_of(&data(), "java.lang.NullPointerException at bingobrewers.Mod");
        assert!(has_solution(&fixes));

        let without: Vec<oneclient_events::CrashFix> = fixes
            .into_iter()
            .filter(|fix| fix.kind != SOLUTION_KIND)
            .collect();

        assert!(!without.is_empty(), "nothing left to test against");
        assert!(!has_solution(&without));
    }

    fn fixes_of(data: &CrashData, document: &str) -> Vec<oneclient_events::CrashFix> {
        data.matches(document)
            .into_iter()
            .map(|hit| oneclient_events::CrashFix {
                text: hit.text,
                kind: hit.kind,
            })
            .collect()
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
