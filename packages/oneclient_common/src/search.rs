//! Query normalisation and typo-tolerant matching for the launcher's search
//! boxes.
//!
//! Every search surface — the package browser, a cluster's package list, the
//! settings index — funnels its raw text box through [`SearchQuery`] so they all
//! agree on what a query *is*. Surfaces that match locally then use the scoring
//! here; surfaces that hand the query to a remote provider only take the
//! normalised text, because ranking a provider's results is the provider's job.

/// Collapses a text box's raw contents into the canonical form every search
/// agrees on: no leading or trailing whitespace, internal runs of whitespace
/// reduced to a single space.
///
/// Without this, `"sodium"`, `" sodium"` and `"sodium  extra"` are three
/// different searches — different provider requests, different cache keys, and
/// for a plain `contains` filter, different results.
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

/// How well a candidate matched. Ordered worst to best, so a list of candidates
/// sorts into relevance order with `sort_by_key(|c| Reverse(score))`.
///
/// The tier dominates the comparison: a prefix match always beats a substring
/// match, which always beats a subsequence match, which always beats a
/// typo-tolerant one. The remainder only orders candidates *within* a tier, so
/// the ordering never surprises — a worse kind of match cannot climb above a
/// better one by scoring well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MatchScore(u32);

impl MatchScore {
    /// What an empty query awards every candidate: nothing is more relevant than
    /// anything else, so whatever order the caller already had survives.
    pub const NEUTRAL: Self = Self(0);
}

/// Tier floors. The gap between them is wider than any bonus below can reach.
const TIER_EXACT: u32 = 5_000;
const TIER_PREFIX: u32 = 4_000;
const TIER_WORD_PREFIX: u32 = 3_000;
const TIER_SUBSTRING: u32 = 2_000;
const TIER_SUBSEQUENCE: u32 = 1_000;
const TIER_TYPO: u32 = 100;

/// Ceiling for the within-tier bonus, comfortably below the 1000-wide tier gap.
const BONUS_MAX: u32 = 900;

/// Below this many characters a typo budget would match almost anything, so
/// short terms are held to the exact tiers.
const MIN_TYPO_LEN: usize = 4;

/// A search box's contents, normalised once so matching a list of candidates
/// against it doesn't redo the work per row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchQuery {
    /// Normalised and lowercased; this is what gets matched and what a remote
    /// provider should be sent.
    text: String,
    /// `text` split on spaces. Multi-word queries fall back to matching each
    /// word independently, so "sodum extra" can still reach "Sodium Extra".
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

    /// The normalised query text. Safe to use as a cache key or to send to a
    /// provider — two raw inputs differing only in spacing produce the same one.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Scores `haystack`, or `None` if it doesn't match at all.
    ///
    /// An empty query matches everything at [`MatchScore::NEUTRAL`].
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

        // The query as one string didn't land, so try it as independent words
        // against the candidate's: every term has to hit one, and the weakest
        // hit sets the tier. This is what lets word order differ ("extra
        // sodium"), what keeps a typo in one word from sinking the whole query,
        // and what gives the typo budget a word to work against instead of a
        // whole name it could never be close enough to.
        let mut worst = u32::MAX;
        for term in &self.terms {
            let best = hay
                .split(' ')
                .filter_map(|word| score_whole(word, term))
                .max()
                .or_else(|| score_whole(&hay, term))?;
            worst = worst.min(best.0);
        }
        // A per-word match is a weaker claim than a whole-query one, so it can
        // never outrank the tier it matched at.
        Some(MatchScore(worst.saturating_sub(1)))
    }

    /// The best score across several fields of the same candidate — a package's
    /// display name and its file name, say. `None` if no field matched.
    #[must_use]
    pub fn best_score<'a>(&self, fields: impl IntoIterator<Item = &'a str>) -> Option<MatchScore> {
        fields.into_iter().filter_map(|f| self.score(f)).max()
    }

    #[must_use]
    pub fn matches(&self, haystack: &str) -> bool {
        self.score(haystack).is_some()
    }
}

/// Scores one already-normalised, already-lowercased pair.
fn score_whole(hay: &str, needle: &str) -> Option<MatchScore> {
    if hay == needle {
        return Some(MatchScore(TIER_EXACT));
    }
    if hay.starts_with(needle) {
        return Some(MatchScore(TIER_PREFIX + coverage(hay, needle)));
    }
    if let Some(at) = hay.find(needle) {
        // Landing on a word boundary ("extra" in "Sodium Extra") reads as a real
        // hit; landing mid-word ("ode" in "Sodium") is closer to a coincidence.
        let word_start = hay[..at].ends_with(' ');
        let tier = if word_start {
            TIER_WORD_PREFIX
        } else {
            TIER_SUBSTRING
        };
        return Some(MatchScore(tier + coverage(hay, needle)));
    }
    if let Some(span) = subsequence_span(hay, needle) {
        // A subsequence spread over the whole name is a weaker match than one
        // packed together, so tightness — not just length — sets the bonus.
        let tightness = scale(needle.chars().count(), span);
        return Some(MatchScore(TIER_SUBSEQUENCE + tightness));
    }

    let needle_len = needle.chars().count();
    if needle_len < MIN_TYPO_LEN {
        return None;
    }
    let budget = typo_budget(needle_len);
    let distance = osa_distance(hay, needle, budget)?;
    // Fewer typos to forgive is a better match; an exact-length hit with one
    // wrong letter should beat one with two.
    let bonus = BONUS_MAX.saturating_sub((distance as u32) * (BONUS_MAX / (budget as u32 + 1)));
    Some(MatchScore(TIER_TYPO + bonus))
}

/// How much of the candidate the query accounts for, as a within-tier bonus.
/// "Sodium" hit by "sodiu" outranks "Sodium Extra Extras" hit by the same.
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

/// Letters a term of this length may get wrong. Deliberately stingy: a generous
/// budget turns every search into a match for everything.
fn typo_budget(len: usize) -> usize {
    if len >= 8 { 2 } else { 1 }
}

/// The width of the tightest window of `hay` containing `needle`'s characters in
/// order, or `None` if it isn't a subsequence at all. Covers the typo class a
/// dropped or doubled letter produces ("sodum", "sodiium").
fn subsequence_span(hay: &str, needle: &str) -> Option<usize> {
    let hay: Vec<char> = hay.chars().collect();
    let needle: Vec<char> = needle.chars().collect();
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }

    // Walk forward for the end of the earliest match, then walk back from there
    // for the latest start that still matches — that pair is the tightest window
    // around this match.
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

/// Optimal string alignment distance, giving up as soon as it exceeds `max`.
/// Unlike a plain Levenshtein it counts a transposition ("soduim") as one edit,
/// which is the typo people actually make when typing quickly.
fn osa_distance(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return None;
    }

    // Three rows rotate through these buffers; `two_ago` is only ever read from
    // row two onwards, by which point it holds real values.
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
        // Every later row is at least this large, so there is no way back under
        // the budget from here.
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
        // Dropped letter, transposition, wrong letter, doubled letter.
        for typo in ["sodum", "soduim", "sodiun", "sodiium"] {
            assert!(SearchQuery::new(typo).matches("Sodium"), "{typo}");
        }
    }

    #[test]
    fn short_terms_are_not_fuzzed() {
        // Two edits away from half the alphabet; a budget here is noise.
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
                // exact, then prefix (shorter first), then word-boundary
                // substring, then the typo-tolerant tail.
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
