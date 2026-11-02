use std::collections::HashMap;
use std::fmt::Display;
use std::sync::LazyLock;

static EN_US: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../lang/en_us.json")).expect("lang/en_us.json is valid")
});

pub fn t(key: &'static str) -> &'static str {
    match EN_US.get(key) {
        Some(value) => value,
        None => {
            tracing::warn!("Missing translation key '{key}'");
            key
        }
    }
}

pub fn tf(key: &'static str, args: &[&dyn Display]) -> String {
    format_placeholders(t(key), args)
}

pub fn format_placeholders(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    let mut next = 0;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        if let Some(after) = tail.strip_prefix('%') {
            out.push('%');
            rest = after;
        } else if let Some(after) = tail.strip_prefix('s') {
            if let Some(arg) = args.get(next) {
                out.push_str(&arg.to_string());
            }
            next += 1;
            rest = after;
        } else if let Some((index, after)) = tail
            .split_once("$s")
            .and_then(|(digits, after)| digits.parse::<usize>().ok().map(|i| (i, after)))
        {
            if let Some(arg) = index.checked_sub(1).and_then(|i| args.get(i)) {
                out.push_str(&arg.to_string());
            }
            rest = after;
        } else {
            out.push('%');
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn fills_sequential_and_positional_placeholders() {
        assert_eq!(format_placeholders("%s of %s", &[&1, &"two"]), "1 of two");
        assert_eq!(
            format_placeholders("%2$s before %1$s", &[&"a", &"b"]),
            "b before a"
        );
        assert_eq!(
            format_placeholders("100%% done, %s", &[&"ok"]),
            "100% done, ok"
        );
        assert_eq!(format_placeholders("50% off", &[]), "50% off");
        assert_eq!(format_placeholders("%s and %s", &[&"one"]), "one and ");
    }

    fn collect_keys(dir: &Path, pattern: &regex::Regex, keys: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect_keys(&path, pattern, keys);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                keys.extend(pattern.captures_iter(&source).map(|c| c[1].to_string()));
            }
        }
    }

    #[test]
    fn source_keys_match_en_us() {
        let pattern = regex::Regex::new(r#"\btf?\(\s*"([^"]+)""#).unwrap();
        let mut used = BTreeSet::new();
        collect_keys(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &pattern,
            &mut used,
        );
        used.retain(|key| key.starts_with("oneclient."));
        let defined: BTreeSet<String> = EN_US.keys().cloned().collect();

        let missing: Vec<_> = used.difference(&defined).collect();
        let unused: Vec<_> = defined.difference(&used).collect();
        assert!(
            missing.is_empty(),
            "keys used but not in en_us.json: {missing:?}"
        );
        assert!(
            unused.is_empty(),
            "keys in en_us.json never used: {unused:?}"
        );
    }
}
