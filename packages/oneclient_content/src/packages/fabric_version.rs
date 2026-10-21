//! Version ordering as Fabric Loader does it, ported from its `VersionParser`,

use std::cmp::Ordering;

struct Semantic<'a> {
    components: Vec<i32>,
    prerelease: Option<&'a str>,
    build: Option<&'a str>,
}

impl Semantic<'_> {
    fn friendly(&self) -> String {
        let mut out = self
            .components
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(".");
        if let Some(prerelease) = self.prerelease {
            out.push('-');
            out.push_str(prerelease);
        }
        if let Some(build) = self.build {
            out.push('+');
            out.push_str(build);
        }
        out
    }
}

fn parse(version: &str) -> Option<Semantic<'_>> {
    let (version, build) = match version.split_once('+') {
        Some((version, build)) => (version, Some(build)),
        None => (version, None),
    };
    let (version, prerelease) = match version.split_once('-') {
        Some((version, prerelease)) => (version, Some(prerelease)),
        None => (version, None),
    };

    if prerelease.is_some_and(|prerelease| !is_dot_separated_id(prerelease)) {
        return None;
    }

    let components = version
        .split('.')
        .map(|component| component.parse::<i32>().ok())
        .collect::<Option<Vec<_>>>()?;

    Some(Semantic {
        components,
        prerelease,
        build,
    })
}

fn is_dot_separated_id(text: &str) -> bool {
    text.is_empty()
        || text.split('.').all(|part| {
            !part.is_empty() && part.bytes().all(|b| b == b'-' || b.is_ascii_alphanumeric())
        })
}

fn is_unsigned_integer(text: &str) -> bool {
    text == "0"
        || (!text.is_empty() && !text.starts_with('0') && text.bytes().all(|b| b.is_ascii_digit()))
}

fn compare_strings(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

fn compare_prereleases(left: &str, right: &str) -> Ordering {
    let mut left = left.split('.').filter(|part| !part.is_empty());
    let mut right = right.split('.').filter(|part| !part.is_empty());

    loop {
        let (a, b) = match (left.next(), right.next()) {
            (Some(a), Some(b)) => (a, b),
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => return Ordering::Equal,
        };

        let by_kind = match (is_unsigned_integer(a), is_unsigned_integer(b)) {
            (true, true) => a.len().cmp(&b.len()),
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => Ordering::Equal,
        };
        let ordering = by_kind.then_with(|| compare_strings(a, b));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
}

#[must_use]
pub fn compare(left: &str, right: &str) -> Ordering {
    let (left, right) = match (parse(left), parse(right)) {
        (Some(left), Some(right)) => (left, right),
        (parsed_left, parsed_right) => {
            let friendly = |parsed: Option<Semantic<'_>>, raw: &str| {
                parsed.map_or_else(|| raw.to_owned(), |semantic| semantic.friendly())
            };
            return compare_strings(&friendly(parsed_left, left), &friendly(parsed_right, right));
        }
    };

    for index in 0..left.components.len().max(right.components.len()) {
        let component =
            |semantic: &Semantic<'_>| semantic.components.get(index).copied().unwrap_or(0);
        let ordering = component(&left).cmp(&component(&right));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }

    match (left.prerelease, right.prerelease) {
        (Some(left), Some(right)) => compare_prereleases(left, right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_order_as_fabric_loader_orders_them() {
        use Ordering::{Equal, Greater, Less};

        for (left, right, expected) in [
            ("0.141.4+1.21.11", "0.141.6+1.21.11", Less),
            ("1.13.12+kotlin.2.4.0", "1.14.1+kotlin.2.4.20", Less),
            ("0.141.10", "0.141.9", Greater),
            ("1.0", "1.0.0", Equal),
            ("1.0.0+a", "1.0.0+b", Equal),
            ("1.0.0-beta.1", "1.0.0", Less),
            ("1.0.0-beta.2", "1.0.0-beta.10", Less),
            ("1.0.0-beta", "1.0.0-beta.1", Less),
            ("1.0.0-1", "1.0.0-alpha", Less),
            (
                "0.4.0-alpha.0.27+1.21.11",
                "0.4.0-alpha.0.3+1.21.11",
                Greater,
            ),
            ("1.01", "1.1", Equal),
            ("1.0.0-", "1.0.0", Less),
            ("1.0.0-", "1.0.0-0", Less),
            ("1.0.0-rc", "1.0.0-rc-1", Less),
            ("1.99999999999", "1.200", Greater),
            ("1..5", "1.0.5", Less),
            ("v1.10", "v1.9", Less),
            ("1.2", "1.2a", Less),
        ] {
            assert_eq!(compare(left, right), expected, "{left} vs {right}");
            assert_eq!(
                compare(right, left),
                expected.reverse(),
                "{right} vs {left}"
            );
        }
    }
}
