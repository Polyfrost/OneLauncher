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
const BUNDLED_LANGUAGE: &str = "zh_TW";

static ZH_TW: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../lang/zh_tw.json")).expect("lang/zh_tw.json is valid")
});

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
    with_bundled_language(AVAILABLE.read().unwrap_or_else(|e| e.into_inner()).clone())
}

fn with_bundled_language(mut languages: Vec<String>) -> Vec<String> {
    languages.retain(|code| !code.eq_ignore_ascii_case(BUNDLED_LANGUAGE));
    languages.push(BUNDLED_LANGUAGE.to_string());
    languages.sort();
    languages
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
        Some(code) if code.eq_ignore_ascii_case(BUNDLED_LANGUAGE) => {
            Some((ZH_TW.len() * 100 / i18n::source().len().max(1)) as u32)
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
    let cached = code
        .and_then(newest_cached)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| parse_translations(&bytes));
    i18n::install(translation_table(code, cached));
    notify_changed();
}

fn translation_table(
    code: Option<&str>,
    cached: Option<HashMap<String, String>>,
) -> Option<HashMap<String, String>> {
    if code.is_some_and(|code| code.eq_ignore_ascii_case(BUNDLED_LANGUAGE)) {
        let mut table = ZH_TW.clone();
        if let Some(cached) = cached {
            table.extend(cached);
        }
        Some(table)
    } else {
        cached
    }
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
    fn traditional_chinese_is_available_without_a_manifest() {
        assert_eq!(with_bundled_language(vec![]), vec!["zh_TW"]);
        assert_eq!(
            with_bundled_language(vec!["zh_TW".into(), "de_DE".into(), "zh_tw".into()]),
            vec!["de_DE", "zh_TW"]
        );
    }

    #[test]
    fn traditional_chinese_covers_source_and_preserves_placeholders() {
        assert_eq!(ZH_TW.len(), i18n::source().len());
        for (key, english) in i18n::source() {
            let translated = ZH_TW.get(key).unwrap_or_else(|| panic!("missing {key}"));
            assert!(!translated.trim().is_empty(), "empty {key}");
            assert_eq!(i18n::arity(english), i18n::arity(translated), "{key}");
        }
    }

    #[test]
    fn traditional_chinese_preserves_each_argument_and_literal_percent() {
        let pattern = regex::Regex::new(r"%%|%s|%(\d+)\$s").unwrap();
        let signature = |text: &str| {
            let mut sequential = 0;
            let mut parameters = BTreeMap::<usize, usize>::new();
            for capture in pattern.captures_iter(text) {
                let index = match &capture[0] {
                    "%%" => 0,
                    "%s" => {
                        sequential += 1;
                        sequential
                    }
                    _ => capture[1].parse().unwrap(),
                };
                *parameters.entry(index).or_default() += 1;
            }
            parameters
        };
        for (key, english) in i18n::source() {
            assert_eq!(signature(english), signature(&ZH_TW[key]), "{key}");
        }
        assert_eq!(
            i18n::format_placeholders(
                &ZH_TW["oneclient.cluster.package_manager.search_empty.scoped.title"],
                &[&"模組", &"本機"],
            ),
            "「本機」中沒有符合搜尋條件的模組。"
        );
    }

    #[test]
    fn cached_translations_overlay_bundled_chinese_without_losing_other_entries() {
        let key = "oneclient.common.cancel";
        let offline = translation_table(Some("zh_TW"), None).unwrap();
        assert_eq!(offline[key], "取消");
        let cached = HashMap::from([(key.to_string(), "取消操作".to_string())]);
        let updated = translation_table(Some("zh_TW"), Some(cached.clone())).unwrap();
        assert_eq!(updated[key], "取消操作");
        assert_eq!(updated.len(), offline.len());
        assert_eq!(
            updated["oneclient.common.save"],
            offline["oneclient.common.save"]
        );
        assert!(translation_table(None, None).is_none());
        assert!(translation_table(Some("de_DE"), None).is_none());
        assert_eq!(
            translation_table(Some("de_DE"), Some(cached.clone())),
            Some(cached)
        );
    }

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
