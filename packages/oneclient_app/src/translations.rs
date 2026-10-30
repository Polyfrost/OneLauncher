use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, SystemTime};

use anyhow::Context;
use freya::prelude::State;
use oneclient_net::RequestClient;
use tokio::sync::{Notify, watch};

use crate::i18n;

const DISTRIBUTION_URL: &str = "https://translations.r2.polyfrost.org/oneclient/v1";
const FILE_NAME: &str = "oneclient.json";
const MANIFEST_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const RETRY_DELAY: Duration = Duration::from_secs(10 * 60);

const NAMES: &[(&str, &str, &str)] = &[
    ("ar_SA", "العربية", "Arabic"),
    ("bg_BG", "Български", "Bulgarian"),
    ("cs_CZ", "Čeština", "Czech"),
    ("da_DK", "Dansk", "Danish"),
    ("de_DE", "Deutsch", "German"),
    ("el_GR", "Ελληνικά", "Greek"),
    ("es_ES", "Español (España)", "Spanish (Spain)"),
    ("es_MX", "Español (México)", "Spanish (Mexico)"),
    ("et_EE", "Eesti", "Estonian"),
    ("fi_FI", "Suomi", "Finnish"),
    ("fil_PH", "Filipino", "Filipino"),
    ("fr_FR", "Français", "French"),
    ("he_IL", "עברית", "Hebrew"),
    ("hr_HR", "Hrvatski", "Croatian"),
    ("hu_HU", "Magyar", "Hungarian"),
    ("id_ID", "Bahasa Indonesia", "Indonesian"),
    ("it_IT", "Italiano", "Italian"),
    ("ja_JP", "日本語", "Japanese"),
    ("ko_KR", "한국어", "Korean"),
    ("lt_LT", "Lietuvių", "Lithuanian"),
    ("lv_LV", "Latviešu", "Latvian"),
    ("nl_NL", "Nederlands", "Dutch"),
    ("no_NO", "Norsk", "Norwegian"),
    ("pl_PL", "Polski", "Polish"),
    ("pt_BR", "Português (Brasil)", "Portuguese (Brazil)"),
    ("pt_PT", "Português (Portugal)", "Portuguese (Portugal)"),
    ("ro_RO", "Română", "Romanian"),
    ("ru_RU", "Русский", "Russian"),
    ("sk_SK", "Slovenčina", "Slovak"),
    ("sl_SI", "Slovenščina", "Slovenian"),
    ("sr_SP", "Српски", "Serbian"),
    ("sv_SE", "Svenska", "Swedish"),
    ("th_TH", "ไทย", "Thai"),
    ("tr_TR", "Türkçe", "Turkish"),
    ("uk_UA", "Українська", "Ukrainian"),
    ("vi_VN", "Tiếng Việt", "Vietnamese"),
    ("zh_CN", "简体中文", "Chinese (Simplified)"),
    ("zh_TW", "繁體中文", "Chinese (Traditional)"),
];

struct Manifest {
    timestamp: u64,
    files: BTreeMap<String, String>,
}

static WANTED: RwLock<Option<String>> = RwLock::new(None);
static AVAILABLE: RwLock<Vec<String>> = RwLock::new(Vec::new());
static WAKE: Notify = Notify::const_new();
static CHANGED: LazyLock<watch::Sender<u64>> = LazyLock::new(|| watch::channel(0).0);

pub fn bind_revision(revision: State<u32>) {
    i18n::bind_revision(revision);
}

pub fn subscribe() -> watch::Receiver<u64> {
    CHANGED.subscribe()
}

pub fn available() -> Vec<String> {
    AVAILABLE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn names(code: Option<&str>) -> (String, String) {
    let Some(code) = code else {
        return ("English".to_string(), "English (US)".to_string());
    };
    NAMES
        .iter()
        .find(|(known, _, _)| known.eq_ignore_ascii_case(code))
        .map_or((code.to_string(), String::new()), |(_, native, english)| {
            (native.to_string(), english.to_string())
        })
}

pub fn choices() -> Vec<Option<String>> {
    std::iter::once(None)
        .chain(available().into_iter().map(Some))
        .collect()
}

pub fn coverage(code: Option<&str>) -> Option<u32> {
    match code {
        None => Some(100),
        Some(code) if WANTED.read().unwrap_or_else(|e| e.into_inner()).as_deref() == Some(code) => {
            i18n::coverage()
        }
        Some(_) => None,
    }
}

pub fn init(language: Option<String>) {
    if let Some(manifest) = cached_manifest() {
        *AVAILABLE.write().unwrap_or_else(|e| e.into_inner()) =
            manifest.0.files.into_keys().collect();
    }
    select(language);
}

pub fn select(language: Option<String>) {
    let language = language.filter(|code| valid_code(code));
    {
        let mut wanted = WANTED.write().unwrap_or_else(|e| e.into_inner());
        if *wanted == language {
            return;
        }
        wanted.clone_from(&language);
    }
    install_cached(language.as_deref());
    WAKE.notify_one();
}

pub fn start(client: RequestClient) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async move {
        loop {
            let delay = match refresh(&client).await {
                Ok(()) => MANIFEST_TTL,
                Err(err) => {
                    tracing::warn!("community translations refresh failed: {err:#}");
                    RETRY_DELAY
                }
            };
            tokio::select! {
                () = tokio::time::sleep(delay) => {}
                () = WAKE.notified() => {}
            }
        }
    });
}

async fn refresh(client: &RequestClient) -> anyhow::Result<()> {
    let manifest_url = format!("{DISTRIBUTION_URL}/manifest.json");
    if oneclient_common::consent::blocks_url(&manifest_url) {
        return Ok(());
    }
    let dir = cache_dir().context("no cache folder")?;

    let manifest = match cached_manifest() {
        Some((manifest, age)) if age < MANIFEST_TTL => manifest,
        _ => {
            let bytes = fetch(client, &manifest_url).await?;
            let manifest = parse_manifest(&bytes).context("invalid manifest")?;
            polyio::write_atomic(dir.join("manifest.json"), &bytes).await?;
            manifest
        }
    };

    let languages: Vec<String> = manifest.files.keys().cloned().collect();
    let listed_changed = {
        let mut available = AVAILABLE.write().unwrap_or_else(|e| e.into_inner());
        let differs = *available != languages;
        *available = languages;
        differs
    };
    if listed_changed {
        notify_changed();
    }

    let Some(code) = WANTED.read().unwrap_or_else(|e| e.into_inner()).clone() else {
        return Ok(());
    };
    let Some(path) = manifest.files.get(&code) else {
        return Ok(());
    };
    let language_dir = dir.join(&code);
    let target = language_dir.join(format!("{}.json", manifest.timestamp));
    if target.is_file() {
        return Ok(());
    }

    let bytes = fetch(
        client,
        &format!("{DISTRIBUTION_URL}{path}?timestamp={}", manifest.timestamp),
    )
    .await?;
    serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes)
        .context("invalid translation file")?;
    polyio::write_atomic(&target, &bytes).await?;
    prune_older(&language_dir, &target);

    if WANTED.read().unwrap_or_else(|e| e.into_inner()).as_deref() == Some(code.as_str()) {
        install_cached(Some(&code));
    }
    Ok(())
}

async fn fetch(client: &RequestClient, url: &str) -> anyhow::Result<Vec<u8>> {
    let response = client
        .http()
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .await?
        .error_for_status()?;
    Ok(response.bytes().await?.to_vec())
}

fn notify_changed() {
    CHANGED.send_modify(|revision| *revision = revision.wrapping_add(1));
}

fn install_cached(code: Option<&str>) {
    let table = code
        .and_then(newest_cached)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| parse_translations(&bytes));
    i18n::install(table);
    notify_changed();
}

fn valid_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 16
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn cache_dir() -> Option<PathBuf> {
    oneclient_common::paths::caches_dir()
        .ok()
        .map(|dir| dir.join("translations"))
}

fn cached_manifest() -> Option<(Manifest, Duration)> {
    let path = cache_dir()?.join("manifest.json");
    let age = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .unwrap_or(Duration::MAX);
    let manifest = parse_manifest(&std::fs::read(path).ok()?)?;
    Some((manifest, age))
}

fn parse_manifest(bytes: &[u8]) -> Option<Manifest> {
    let json: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let timestamp = json.get("timestamp")?.as_u64()?;
    let content = json.get("content")?.as_object()?;
    let files = content
        .iter()
        .filter(|(code, _)| valid_code(code))
        .filter_map(|(code, paths)| {
            let path = paths
                .as_array()?
                .iter()
                .filter_map(|p| p.as_str())
                .find(|path| {
                    path.starts_with("/translations/")
                        && path.ends_with(&format!("/{FILE_NAME}"))
                        && !path.contains("..")
                })?;
            Some((code.clone(), path.to_string()))
        })
        .collect();
    Some(Manifest { timestamp, files })
}

fn parse_translations(bytes: &[u8]) -> Option<HashMap<String, String>> {
    let parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(bytes).ok()?;
    let source = i18n::source();
    let table: HashMap<String, String> = parsed
        .into_iter()
        .filter_map(|(key, value)| {
            let value = value.as_str()?.to_string();
            let english = source.get(&key)?;
            (i18n::arity(english) == i18n::arity(&value)).then_some((key, value))
        })
        .collect();
    (!table.is_empty()).then_some(table)
}

fn release_of(path: &Path) -> Option<u64> {
    path.file_name()?
        .to_str()?
        .strip_suffix(".json")?
        .parse()
        .ok()
}

fn newest_cached(code: &str) -> Option<PathBuf> {
    std::fs::read_dir(cache_dir()?.join(code))
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter_map(|path| release_of(&path).map(|release| (release, path)))
        .max_by_key(|(release, _)| *release)
        .map(|(_, path)| path)
}

fn prune_older(dir: &Path, current: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.filter_map(|entry| entry.ok().map(|entry| entry.path())) {
        if path != current
            && release_of(&path).is_some()
            && let Err(err) = std::fs::remove_file(&path)
        {
            tracing::warn!(path = %path.display(), "could not prune old translations: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_keeps_only_safe_launcher_files() {
        let manifest = parse_manifest(
            br#"{"timestamp":5,"hash":"x","content":{
                "pl_PL":["/translations/pl_PL/oneclient.json"],
                "de_DE":["/translations/de_DE/other.json"],
                "../x":["/translations/x/oneclient.json"],
                "fr_FR":["/translations/../oneclient.json"]
            }}"#,
        )
        .unwrap();
        assert_eq!(manifest.timestamp, 5);
        assert_eq!(
            manifest.files.into_iter().collect::<Vec<_>>(),
            vec![(
                "pl_PL".to_string(),
                "/translations/pl_PL/oneclient.json".to_string()
            )]
        );
    }

    #[test]
    fn translations_drop_unknown_keys_and_wrong_placeholders() {
        let table = parse_translations(
            br#"{
                "oneclient.common.cancel":"Anuluj",
                "oneclient.components.package_row.copies":"kopie",
                "oneclient.not.a.key":"x",
                "oneclient.cluster.settings.migrate.from":"Z %s"
            }"#,
        )
        .unwrap();
        assert_eq!(table.get("oneclient.common.cancel").unwrap(), "Anuluj");
        assert_eq!(
            table
                .get("oneclient.cluster.settings.migrate.from")
                .unwrap(),
            "Z %s"
        );
        assert!(!table.contains_key("oneclient.components.package_row.copies"));
        assert!(!table.contains_key("oneclient.not.a.key"));
    }
}
