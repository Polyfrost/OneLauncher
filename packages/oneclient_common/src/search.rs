//! Local surfaces use the scoring here surfaces backed by a remote provider
//! take only the normalised text since ranking their results is their job

/// Trims and collapses internal whitespace runs so spacing variants share one
/// provider request cache key and filter result
#[must_use]
pub fn normalize_query(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for word in raw.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Ordered worst to best
/// The tier dominates the within-tier bonus can never lift a weaker kind of
/// match above a stronger one
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MatchScore(u32);

impl MatchScore {
    /// Awarded to every candidate by an empty query preserving the caller's order
    pub const NEUTRAL: Self = Self(0);
}

/// Tier floors
/// The gap between them is wider than any bonus below can reach
const TIER_EXACT: u32 = 5_000;
const TIER_PREFIX: u32 = 4_000;
const TIER_WORD_PREFIX: u32 = 3_000;
const TIER_SUBSTRING: u32 = 2_000;
const TIER_SUBSEQUENCE: u32 = 1_000;
const TIER_TYPO: u32 = 100;

/// Ceiling for the within-tier bonus comfortably below the 1000-wide tier gap
const BONUS_MAX: u32 = 900;

/// Below this many characters a typo budget would match almost anything so
/// short terms are held to the exact tiers
const MIN_TYPO_LEN: usize = 4;

/// Normalised once so matching a list of candidates doesn't redo the work per row
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchQuery {
    text: String,
    terms: Vec<String>,
}

impl SearchQuery {
    #[must_use]
    pub fn new(raw: &str) -> Self {
        let text = normalize_query(raw).to_lowercase();
        let terms = text
            .split(' ')
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect();
        Self { text, terms }
    }

    /// Safe as a cache key inputs differing only in spacing produce the same text
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// `None` if it doesn't match
    /// An empty query matches everything at [`MatchScore::NEUTRAL`]
    #[must_use]
    pub fn score(&self, haystack: &str) -> Option<MatchScore> {
        if self.is_empty() {
            return Some(MatchScore::NEUTRAL);
        }

        let hay = normalize_query(haystack).to_lowercase();
        if hay.is_empty() {
            return None;
        }

        if let Some(score) = score_whole(&hay, &self.text) {
            return Some(score);
        }

        // Per-word fallback lets word order differ keeps one typo from sinking
        // the query and gives the typo budget a word rather than a whole name
        let mut worst = u32::MAX;
        for term in &self.terms {
            let best = hay
                .split(' ')
                .filter_map(|word| score_whole(word, term))
                .max()
                .or_else(|| score_whole(&hay, term))?;
            worst = worst.min(best.0);
        }
        // A per-word match must never outrank the tier it matched at
        Some(MatchScore(worst.saturating_sub(1)))
    }

    #[must_use]
    pub fn best_score<'a>(&self, fields: impl IntoIterator<Item = &'a str>) -> Option<MatchScore> {
        fields.into_iter().filter_map(|f| self.score(f)).max()
    }

    #[must_use]
    pub fn matches(&self, haystack: &str) -> bool {
        self.score(haystack).is_some()
    }
}

/// Both inputs must already be normalised and lowercased
fn score_whole(hay: &str, needle: &str) -> Option<MatchScore> {
    if hay == needle {
        return Some(MatchScore(TIER_EXACT));
    }
    if hay.starts_with(needle) {
        return Some(MatchScore(TIER_PREFIX + coverage(hay, needle)));
    }
    if let Some(at) = hay.find(needle) {
        // A word-boundary hit reads as intentional mid-word is closer to coincidence
        let word_start = hay[..at].ends_with(' ');
        let tier = if word_start {
            TIER_WORD_PREFIX
        } else {
            TIER_SUBSTRING
        };
        return Some(MatchScore(tier + coverage(hay, needle)));
    }
    if let Some(span) = subsequence_span(hay, needle) {
        // Tightness not just length sets the bonus a scattered subsequence is weaker
        let tightness = scale(needle.chars().count(), span);
        return Some(MatchScore(TIER_SUBSEQUENCE + tightness));
    }

    let needle_len = needle.chars().count();
    if needle_len < MIN_TYPO_LEN {
        return None;
    }
    let budget = typo_budget(needle_len);
    let distance = osa_distance(hay, needle, budget)?;
    // Fewer typos forgiven ranks higher
    let bonus = BONUS_MAX.saturating_sub((distance as u32) * (BONUS_MAX / (budget as u32 + 1)));
    Some(MatchScore(TIER_TYPO + bonus))
}

/// Within-tier bonus for how much of the candidate the query accounts for
fn coverage(hay: &str, needle: &str) -> u32 {
    scale(needle.chars().count(), hay.chars().count())
}

fn scale(part: usize, whole: usize) -> u32 {
    if whole == 0 {
        return 0;
    }
    let ratio = (part.min(whole) as u64 * u64::from(BONUS_MAX)) / whole as u64;
    ratio as u32
}

/// Letters a term of this length may get wrong
/// Deliberately stingy a generous budget makes every query match everything
fn typo_budget(len: usize) -> usize {
    if len >= 8 { 2 } else { 1 }
}

/// Width of the tightest window of `hay` holding `needle`'s chars in order or
/// `None` if not a subsequence
/// Covers dropped/doubled-letter typos
fn subsequence_span(hay: &str, needle: &str) -> Option<usize> {
    let hay: Vec<char> = hay.chars().collect();
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }

    // Earliest end then latest start walking back from it the tightest window
    let mut n = 0;
    let mut end = None;
    for (i, c) in hay.iter().enumerate() {
        if *c == needle[n] {
            n += 1;
            if n == needle.len() {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;

    let mut n = needle.len();
    let mut start = 0;
    for i in (0..=end).rev() {
        if hay[i] == needle[n - 1] {
            n -= 1;
            if n == 0 {
                start = i;
                break;
            }
        }
    }
    Some(end - start + 1)
}

/// Optimal string alignment distance giving up once it exceeds `max`
/// Unlike Levenshtein a transposition costs one edit
fn osa_distance(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return None;
    }

    // Three rows rotate through these buffers `two_ago` is only read from row
    // two onwards by which point it holds real values
    let mut two_ago: Vec<usize> = vec![0; b.len() + 1];
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur: Vec<usize> = vec![0; b.len() + 1];

    for i in 1..=a.len() {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(two_ago[j - 2] + 1);
            }
            cur[j] = best;
            row_min = row_min.min(best);
        }
        // Every later row is at least this large so the budget is unrecoverable
        if row_min > max {
            return None;
        }
        std::mem::swap(&mut two_ago, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }

    (prev[b.len()] <= max).then_some(prev[b.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn best(query: &str, candidates: &[&str]) -> Vec<String> {
        let q = SearchQuery::new(query);
        let mut scored: Vec<(MatchScore, &str)> = candidates
            .iter()
            .filter_map(|c| q.score(c).map(|s| (s, *c)))
            .collect();
        scored.sort_by_key(|(s, c)| (std::cmp::Reverse(*s), *c));
        scored.into_iter().map(|(_, c)| c.to_string()).collect()
    }

    #[test]
    fn whitespace_never_changes_a_query() {
        let canonical = SearchQuery::new("sodium extra");
        for raw in [
            "  sodium extra",
            "sodium extra  ",
            "sodium   extra",
            "\tsodium\nextra ",
        ] {
            assert_eq!(SearchQuery::new(raw), canonical, "{raw:?}");
        }
        assert_eq!(canonical.as_str(), "sodium extra");
    }

    #[test]
    fn a_blank_query_is_empty() {
        assert!(SearchQuery::new("   ").is_empty());
        assert_eq!(
            SearchQuery::new("").score("anything"),
            Some(MatchScore::NEUTRAL)
        );
    }

    #[test]
    fn common_typos_still_find_the_package() {
        for typo in ["sodum", "soduim", "sodiun", "sodiium"] {
            assert!(SearchQuery::new(typo).matches("Sodium"), "{typo}");
        }
    }

    #[test]
    fn short_terms_are_not_fuzzed() {
        assert!(!SearchQuery::new("abc").matches("Sodium"));
    }

    #[test]
    fn unrelated_names_do_not_match() {
        assert!(!SearchQuery::new("sodium").matches("Journeymap"));
    }

    #[test]
    fn better_kinds_of_match_rank_first() {
        assert_eq!(
            best(
                "sodium",
                &[
                    "Sodium",
                    "Sodium Extra",
                    "Indium (Sodium addon)",
                    "Sodum Fork",
                    "Sodiun"
                ]
            ),
            [
                "Sodium",
                "Sodium Extra",
                "Indium (Sodium addon)",
                "Sodiun",
                "Sodum Fork",
            ]
        );
    }

    #[test]
    fn a_tight_subsequence_beats_a_scattered_one() {
        assert_eq!(
            best("srr", &["Sodium Rendering Regression", "Sorry"]),
            ["Sorry", "Sodium Rendering Regression"]
        );
    }

    #[test]
    fn multi_word_queries_match_out_of_order() {
        let q = SearchQuery::new("extra sodium");
        assert!(q.matches("Sodium Extra"));
        assert!(!q.matches("Sodium"));
    }

    #[test]
    fn a_typo_in_one_word_does_not_sink_the_query() {
        assert!(SearchQuery::new("sodum extra").matches("Sodium Extra"));
    }

    #[test]
    fn best_score_picks_the_strongest_field() {
        let q = SearchQuery::new("sodium");
        let whole = q.score("Sodium").unwrap();
        assert_eq!(q.best_score(["Fabric API", "Sodium"]), Some(whole));
        assert_eq!(q.best_score(["Fabric API", "fabric-api.jar"]), None);
    }
}
