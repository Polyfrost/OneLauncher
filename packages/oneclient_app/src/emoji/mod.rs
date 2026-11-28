mod catalog;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use freya::engine::prelude::{
    FontCollection, FontMgr, ParagraphBuilder, ParagraphStyle, TextStyle,
};
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use catalog::CATALOG;

const MAX_RECENTS: usize = 18;
const RECENTS_FILE: &str = "emoji_recents.json";

const SPLIT_WIDTH_RATIO: f32 = 1.5;

pub(crate) static SHORTCODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":([a-z0-9_+-]+):").expect("valid shortcode regex"));

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct EmojiEntry {
    pub glyph: &'static str,
    aliases: &'static [&'static str],
    extra: &'static [&'static str],
}

impl EmojiEntry {
    pub fn alias(&self) -> &'static str {
        self.aliases
            .iter()
            .min_by_key(|alias| alias.len())
            .copied()
            .unwrap_or_default()
    }

    fn all_aliases(&self) -> impl Iterator<Item = &'static str> {
        self.aliases.iter().chain(self.extra).copied()
    }
}

fn all_entries() -> impl Iterator<Item = EmojiEntry> {
    CATALOG.iter().map(|&(glyph, aliases, extra)| EmojiEntry {
        glyph,
        aliases,
        extra,
    })
}

static BY_SHORTCODE: LazyLock<HashMap<&'static str, EmojiEntry>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for entry in all_entries() {
        for alias in entry.aliases {
            map.entry(*alias).or_insert(entry);
        }
    }
    for entry in all_entries() {
        for alias in entry.extra {
            map.entry(*alias).or_insert(entry);
        }
    }
    map
});

#[must_use]
pub(crate) fn emoji_for_shortcode(alias: &str) -> Option<&'static str> {
    BY_SHORTCODE.get(alias).map(|entry| entry.glyph)
}

#[must_use]
pub(crate) fn canonical_shortcodes(text: &str) -> String {
    SHORTCODE
        .replace_all(text, |c: &Captures| match BY_SHORTCODE.get(&c[1]) {
            Some(entry) if entry.extra.contains(&&c[1]) => format!(":{}:", entry.alias()),
            _ => c[0].to_string(),
        })
        .into_owned()
}

static RENDERABLE: LazyLock<Vec<EmojiEntry>> = LazyLock::new(|| {
    let mut fonts = FontCollection::new();
    fonts.set_default_font_manager(FontMgr::default(), None);
    let measure = |text: &str| {
        let mut builder = ParagraphBuilder::new(&ParagraphStyle::new(), fonts.clone());
        let mut style = TextStyle::new();
        style.set_font_size(20.);
        builder.push_style(&style);
        builder.add_text(text);
        let mut paragraph = builder.build();
        paragraph.layout(f32::MAX);
        let resolved = paragraph
            .unresolved_glyphs()
            .is_none_or(|missing| missing == 0);
        resolved.then(|| paragraph.max_intrinsic_width())
    };
    let Some(reference) = measure("😀").filter(|width| *width > 0.) else {
        return Vec::new();
    };
    all_entries()
        .filter(|entry| {
            measure(entry.glyph).is_some_and(|width| width <= reference * SPLIT_WIDTH_RATIO)
        })
        .collect()
});

pub(crate) fn catalog() -> &'static [EmojiEntry] {
    &RENDERABLE
}

pub(crate) fn search(query: &str) -> Vec<EmojiEntry> {
    search_in(catalog(), query)
}

fn search_in(entries: &[EmojiEntry], query: &str) -> Vec<EmojiEntry> {
    let query = query.trim().to_lowercase();
    let query = query.trim_start_matches(':').trim_end_matches(':');
    if query.is_empty() {
        return entries.to_vec();
    }
    let (prefixed, contained): (Vec<_>, Vec<_>) = entries
        .iter()
        .filter(|entry| entry.all_aliases().any(|alias| alias.contains(query)))
        .partition(|entry| entry.all_aliases().any(|alias| alias.starts_with(query)));
    prefixed.into_iter().chain(contained).copied().collect()
}

pub(crate) fn default_entries() -> Vec<EmojiEntry> {
    let recents = recents();
    let mut entries = recents.clone();
    entries.extend(
        catalog()
            .iter()
            .filter(|entry| !recents.iter().any(|recent| recent.glyph == entry.glyph)),
    );
    entries
}

#[derive(Serialize, Deserialize, Default)]
struct RecentsState {
    #[serde(default)]
    recents: Vec<String>,
}

static RECENTS: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| {
    let recents = recents_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str::<RecentsState>(&json).ok())
        .map(|state| state.recents)
        .unwrap_or_default();
    Mutex::new(
        recents
            .into_iter()
            .filter(|alias| resolve(alias).is_some())
            .take(MAX_RECENTS)
            .collect(),
    )
});

fn recents_path() -> Option<PathBuf> {
    oneclient_common::paths::config_dir()
        .ok()
        .map(|dir| dir.join(RECENTS_FILE))
}

fn resolve(alias: &str) -> Option<EmojiEntry> {
    let glyph = BY_SHORTCODE.get(alias)?.glyph;
    catalog().iter().find(|entry| entry.glyph == glyph).copied()
}

fn recents() -> Vec<EmojiEntry> {
    RECENTS
        .lock()
        .map(|recents| recents.iter().filter_map(|alias| resolve(alias)).collect())
        .unwrap_or_default()
}

pub(crate) fn record_recent(entry: EmojiEntry) {
    let Ok(mut recents) = RECENTS.lock() else {
        return;
    };
    let alias = entry.alias();
    recents.retain(|recent| recent != alias);
    recents.insert(0, alias.to_string());
    recents.truncate(MAX_RECENTS);
    let state = RecentsState {
        recents: recents.clone(),
    };
    drop(recents);
    if let (Some(path), Ok(json)) = (recents_path(), serde_json::to_string(&state))
        && let Err(err) = std::fs::write(&path, json)
    {
        tracing::warn!("could not save emoji recents to {}: {err}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(entry: &EmojiEntry, pred: impl Fn(&str) -> bool) -> bool {
        entry.all_aliases().any(pred)
    }

    #[test]
    fn catalog_matches_the_in_game_set() {
        assert_eq!(CATALOG.len(), 753);
        let mut glyphs: Vec<_> = CATALOG.iter().map(|(glyph, _, _)| *glyph).collect();
        glyphs.sort_unstable();
        glyphs.dedup();
        assert_eq!(glyphs.len(), CATALOG.len());
        assert!(CATALOG.iter().all(|(_, aliases, _)| !aliases.is_empty()));
    }

    #[test]
    fn catalog_follows_the_unicode_category_order() {
        let first: Vec<_> = all_entries().take(3).map(|entry| entry.glyph).collect();
        assert_eq!(first, ["😀", "😃", "😄"]);
        let at = |glyph: &str| {
            all_entries()
                .position(|entry| entry.glyph == glyph)
                .unwrap()
        };
        assert!(at("💀") < at("🍎") && at("🍎") < at("♈"));
    }

    #[test]
    fn primary_alias_is_the_shortest_datasource_name() {
        let thumbs = all_entries().find(|entry| entry.glyph == "👍").unwrap();
        assert_eq!(thumbs.alias(), "+1");
        let rofl = all_entries().find(|entry| entry.glyph == "🤣").unwrap();
        assert_eq!(rofl.alias(), "rolling_on_the_floor_laughing");
    }

    #[test]
    fn short_and_long_shortcodes_resolve_to_the_same_emoji() {
        assert_eq!(emoji_for_shortcode("rofl"), Some("🤣"));
        assert_eq!(
            emoji_for_shortcode("rolling_on_the_floor_laughing"),
            Some("🤣")
        );
        assert_eq!(emoji_for_shortcode("slight_smile"), Some("🙂"));
        assert_eq!(
            emoji_for_shortcode("face_with_open_eyes_and_hand_over_mouth"),
            Some("🫢")
        );
        assert_eq!(emoji_for_shortcode("nope"), None);
    }

    #[test]
    fn extra_shortcodes_are_sent_as_datasource_ones() {
        assert_eq!(
            canonical_shortcodes("lol :rofl: :joy: :nope: 12:30:45"),
            "lol :rolling_on_the_floor_laughing: :joy: :nope: 12:30:45"
        );
    }

    #[test]
    fn search_puts_prefix_matches_before_substring_matches() {
        let entries: Vec<_> = all_entries().collect();
        let found = search_in(&entries, ":Fire:");
        assert!(has(found.first().unwrap(), |alias| alias.starts_with("fire")));
        let prefix_end = found
            .iter()
            .position(|entry| !has(entry, |alias| alias.starts_with("fire")))
            .unwrap_or(found.len());
        assert!(
            found[prefix_end..]
                .iter()
                .all(|entry| has(entry, |alias| alias.contains("fire")))
        );
        assert_eq!(
            search_in(&entries, "rofl").first().map(|entry| entry.glyph),
            Some("🤣")
        );
    }
}
