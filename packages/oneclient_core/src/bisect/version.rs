use std::cmp::Ordering;
use std::fmt;

const WILDCARD: i64 = -1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Version {
    Semantic(Semantic),
    Text(String),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Semantic {
    components: Vec<i64>,
    prerelease: Option<String>,
}

impl Version {
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        Self::parse_with(raw, false).unwrap_or_else(|| Self::Text(raw.to_string()))
    }

    fn parse_with(raw: &str, wildcards: bool) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        Some(match Semantic::parse(raw, wildcards) {
            Some(semantic) => Self::Semantic(semantic),
            None => Self::Text(raw.to_string()),
        })
    }

    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Semantic(a), Self::Semantic(b)) => a.compare(b),
            (Self::Unknown, Self::Unknown) => Ordering::Equal,
            (Self::Unknown, _) => Ordering::Less,
            (_, Self::Unknown) => Ordering::Greater,
            _ => self.to_string().cmp(&other.to_string()),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Semantic(semantic) => semantic.fmt(f),
            Self::Text(text) => f.write_str(text),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}

impl Semantic {
    fn parse(raw: &str, wildcards: bool) -> Option<Self> {
        let core = raw.split_once('+').map_or(raw, |(core, _)| core);
        let (core, prerelease) = match core.split_once('-') {
            Some((core, pre)) => {
                let valid = pre.is_empty()
                    || pre.split('.').all(|part| {
                        !part.is_empty()
                            && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                    });
                if !valid {
                    return None;
                }
                (core, Some(pre.to_string()))
            }
            None => (core, None),
        };
        if core.is_empty() || core.starts_with('.') || core.ends_with('.') {
            return None;
        }

        let mut components = Vec::new();
        let mut wildcard_at = None;
        for (index, part) in core.split('.').enumerate() {
            let is_wildcard = matches!(part, "x" | "X" | "*");
            if is_wildcard {
                if !wildcards || prerelease.is_some() || index == 0 {
                    return None;
                }
                components.push(WILDCARD);
                wildcard_at.get_or_insert(index);
                continue;
            }
            if wildcard_at.is_some() || part.is_empty() || !part.chars().all(|c| c.is_ascii_digit())
            {
                return None;
            }
            components.push(part.parse().ok()?);
        }
        if let Some(at) = wildcard_at {
            components.truncate(at + 1);
        }
        Some(Self {
            components,
            prerelease,
        })
    }

    fn component(&self, position: usize) -> i64 {
        match self.components.get(position) {
            Some(value) => *value,
            None if self.components.last() == Some(&WILDCARD) => WILDCARD,
            None => 0,
        }
    }

    fn has_wildcard(&self) -> bool {
        self.components.contains(&WILDCARD)
    }

    fn compare(&self, other: &Self) -> Ordering {
        for index in 0..self.components.len().max(other.components.len()) {
            let (a, b) = (self.component(index), other.component(index));
            if a == WILDCARD || b == WILDCARD {
                continue;
            }
            match a.cmp(&b) {
                Ordering::Equal => {}
                unequal => return unequal,
            }
        }
        match (&self.prerelease, &other.prerelease) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => compare_prerelease(a, b),
        }
    }
}

fn compare_prerelease(a: &str, b: &str) -> Ordering {
    let ids_a: Vec<&str> = if a.is_empty() {
        Vec::new()
    } else {
        a.split('.').collect()
    };
    let ids_b: Vec<&str> = if b.is_empty() {
        Vec::new()
    } else {
        b.split('.').collect()
    };
    for (x, y) in ids_a.iter().zip(&ids_b) {
        let order = match (numeric(x), numeric(y)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    ids_a.len().cmp(&ids_b.len())
}

fn numeric(id: &str) -> Option<u64> {
    let valid = !id.is_empty()
        && id.chars().all(|c| c.is_ascii_digit())
        && (id == "0" || !id.starts_with('0'));
    if valid { id.parse().ok() } else { None }
}

impl fmt::Display for Semantic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self
            .components
            .iter()
            .map(|c| {
                if *c == WILDCARD {
                    "x".to_string()
                } else {
                    c.to_string()
                }
            })
            .collect();
        f.write_str(&parts.join("."))?;
        if let Some(pre) = &self.prerelease {
            write!(f, "-{pre}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
    Ge,
    Gt,
    Le,
    Lt,
    Caret,
    Tilde,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Term {
    op: Op,
    version: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Predicate {
    terms: Vec<Term>,
}

impl Predicate {
    #[must_use]
    pub fn any() -> Self {
        Self { terms: Vec::new() }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() || raw == "*" {
            return Some(Self::any());
        }
        let mut terms = Vec::new();
        for part in raw.split_whitespace() {
            let (op, rest) = [
                (">=", Op::Ge),
                ("<=", Op::Le),
                (">", Op::Gt),
                ("<", Op::Lt),
                ("^", Op::Caret),
                ("~", Op::Tilde),
                ("=", Op::Eq),
            ]
            .into_iter()
            .find_map(|(prefix, op)| part.strip_prefix(prefix).map(|rest| (op, rest)))
            .unwrap_or((Op::Eq, part));
            let version = Version::parse_with(rest, true)?;
            if matches!(version, Version::Text(_)) && op != Op::Eq {
                return None;
            }
            terms.push(Term { op, version });
        }
        Some(Self { terms })
    }

    #[must_use]
    pub fn is_any(&self) -> bool {
        self.terms.is_empty()
    }

    #[must_use]
    pub fn test(&self, version: &Version) -> bool {
        *version == Version::Unknown || self.terms.iter().all(|term| term.test(version))
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.terms.is_empty() {
            return f.write_str("*");
        }
        let parts: Vec<String> = self
            .terms
            .iter()
            .map(|term| {
                let op = match term.op {
                    Op::Eq => "",
                    Op::Ge => ">=",
                    Op::Gt => ">",
                    Op::Le => "<=",
                    Op::Lt => "<",
                    Op::Caret => "^",
                    Op::Tilde => "~",
                };
                format!("{op}{}", term.version)
            })
            .collect();
        f.write_str(&parts.join(" "))
    }
}

impl Term {
    fn test(&self, version: &Version) -> bool {
        let Version::Semantic(base) = &self.version else {
            return version.compare(&self.version) == Ordering::Equal;
        };
        let (base, op) = if base.has_wildcard() {
            let kept: Vec<i64> = base
                .components
                .iter()
                .copied()
                .take_while(|c| *c != WILDCARD)
                .collect();
            let op = if kept.len() == 1 {
                Op::Caret
            } else {
                Op::Tilde
            };
            (
                Semantic {
                    components: kept,
                    prerelease: Some(String::new()),
                },
                op,
            )
        } else {
            (base.clone(), self.op)
        };

        let cmp = |bound: &Semantic| version.compare(&Version::Semantic(bound.clone()));
        match op {
            Op::Eq => cmp(&base) == Ordering::Equal,
            Op::Ge => cmp(&base) != Ordering::Less,
            Op::Gt => cmp(&base) == Ordering::Greater,
            Op::Le => cmp(&base) != Ordering::Greater,
            Op::Lt => cmp(&base) == Ordering::Less,
            Op::Caret | Op::Tilde => {
                let mut low = base.components.clone();
                low.resize(low.len().max(3), 0);
                let mut high = low.clone();
                let bump = usize::from(op == Op::Tilde);
                high[bump] += 1;
                for value in high.iter_mut().skip(bump + 1) {
                    *value = 0;
                }
                let low = Semantic {
                    components: low,
                    prerelease: base.prerelease.clone(),
                };
                let high = Semantic {
                    components: high,
                    prerelease: Some(String::new()),
                };
                cmp(&low) != Ordering::Less && cmp(&high) == Ordering::Less
            }
        }
    }
}

#[must_use]
pub fn maven_range(raw: &str) -> Vec<Predicate> {
    let raw = raw.trim();
    if raw.is_empty() || raw == "*" {
        return vec![Predicate::any()];
    }
    if !raw.contains(['[', '(']) {
        return vec![Predicate::any()];
    }

    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find(['[', '(']) {
        let Some(len) = rest[start..].find([']', ')']) else {
            break;
        };
        let end = start + len;
        let low_inclusive = rest.as_bytes()[start] == b'[';
        let high_inclusive = rest.as_bytes()[end] == b']';
        let inner = &rest[start + 1..end];
        rest = &rest[end + 1..];

        let mut terms = Vec::new();
        match inner.split_once(',') {
            None => {
                let version = Version::parse(&maven_version(inner));
                terms.push(Term {
                    op: Op::Eq,
                    version,
                });
            }
            Some((low, high)) => {
                if !low.trim().is_empty() {
                    terms.push(Term {
                        op: if low_inclusive { Op::Ge } else { Op::Gt },
                        version: Version::parse(&maven_version(low)),
                    });
                }
                if !high.trim().is_empty() {
                    terms.push(Term {
                        op: if high_inclusive { Op::Le } else { Op::Lt },
                        version: Version::parse(&maven_version(high)),
                    });
                }
            }
        }
        if terms
            .iter()
            .any(|term| matches!(term.version, Version::Text(_)) && term.op != Op::Eq)
        {
            out.push(Predicate::any());
        } else {
            out.push(Predicate { terms });
        }
    }
    if out.is_empty() {
        out.push(Predicate::any());
    }
    out
}

#[must_use]
pub fn maven_version(raw: &str) -> String {
    let raw = raw.trim();
    let split = raw
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(raw.len());
    let numeric = raw[..split].trim_end_matches('.');
    if numeric.is_empty() {
        return raw.to_string();
    }
    let mut parts: Vec<String> = numeric
        .split('.')
        .map(|p| {
            p.parse::<u64>()
                .map_or_else(|_| p.to_string(), |n| n.to_string())
        })
        .collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p == "0") {
        parts.pop();
    }
    let base = parts.join(".");
    let qualifier: String = raw[split..]
        .trim_start_matches(['-', '.', '_'])
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '.' })
        .collect();
    let qualifier = qualifier.trim_matches('.');
    match qualifier {
        "" | "ga" | "final" | "release" => base,
        q if [
            "a",
            "alpha",
            "b",
            "beta",
            "m",
            "milestone",
            "rc",
            "cr",
            "snapshot",
            "pre",
        ]
        .iter()
        .any(|known| q.starts_with(known)) =>
        {
            format!("{base}-{}", q.replace("..", "."))
        }
        q => {
            let mut post: Vec<&str> = base.split('.').collect();
            post.resize(post.len().max(3), "0");
            format!("{}.0.1-{}", post.join("."), q.replace("..", "."))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(predicate: &str, version: &str) -> bool {
        Predicate::parse(predicate)
            .unwrap()
            .test(&Version::parse(version))
    }

    #[test]
    fn predicates_follow_fabric_rules() {
        assert!(ok("*", "1.0"));
        assert!(ok(">=1.2", "1.2.0"));
        assert!(!ok(">=1.2", "1.1.9"));
        assert!(ok("^1.2.3", "1.9.0"));
        assert!(!ok("^1.2.3", "2.0.0"));
        assert!(ok("~1.2.3", "1.2.9"));
        assert!(!ok("~1.2.3", "1.3.0"));
        assert!(ok("1.20.x", "1.20.4"));
        assert!(!ok("1.20.x", "1.21"));
        assert!(ok(">=1.0 <2.0", "1.5"));
        assert!(!ok(">=1.0 <2.0", "2.0"));
        assert!(!ok("^1.0.0", "1.0.0-beta.1"));
        assert!(ok("abc", "abc"));
    }

    #[test]
    fn maven_ranges_become_predicates() {
        let test = |range: &str, version: &str| {
            maven_range(range)
                .iter()
                .any(|p| p.test(&Version::parse(&maven_version(version))))
        };
        assert!(test("[1.0,2.0)", "1.5"));
        assert!(!test("[1.0,2.0)", "2.0"));
        assert!(test("[1.0,)", "47.1.3"));
        assert!(test("(,1.0]", "1.0"));
        assert!(test("[1.0]", "1.0.0"));
        assert!(test("1.0", "0.1"));
        assert!(test("[1.0,1.2),[1.5,)", "1.6"));
        assert!(!test("[1.0,1.2),[1.5,)", "1.3"));
        assert!(!test("[1.0,)", "1.0-beta"));
    }
}
