//! Reads the INI files MultiMC-family launchers write (`instance.cfg`,
//! `prismlauncher.cfg`)
//!
//! Two dialects show up in the wild: the old MultiMC format (bare
//! `key=value` lines, backslash escapes) and the QSettings format newer Prism
//! builds save (a `[General]` section, values quoted when they hold special
//! characters, `\xNNNN` for non-ASCII). One lenient reader handles both;
//! sections are ignored since every key these launchers use is unique

use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct Ini {
    values: HashMap<String, String>,
}

impl Ini {
    pub fn parse(text: &str) -> Self {
        let mut values = HashMap::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with(['#', ';', '[']) {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            values.insert(key.trim().to_string(), unescape(value.trim()));
        }
        Self { values }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty() && *value != "@Invalid()")
    }

    pub fn flag(&self, key: &str) -> bool {
        self.get(key)
            .is_some_and(|value| value.eq_ignore_ascii_case("true") || value == "1")
    }

    pub fn number<T: std::str::FromStr>(&self, key: &str) -> Option<T> {
        self.get(key)?.parse().ok()
    }
}

fn unescape(value: &str) -> String {
    let quoted = value.len() >= 2 && value.starts_with('"') && value.ends_with('"');
    let body = if quoted {
        &value[1..value.len() - 1]
    } else {
        value
    };

    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('x') => {
                let mut hex = String::new();
                while hex.len() < 4
                    && let Some(d) = chars.peek().copied().filter(char::is_ascii_hexdigit)
                {
                    hex.push(d);
                    chars.next();
                }
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(decoded) => out.push(decoded),
                    None => {
                        out.push_str("\\x");
                        out.push_str(&hex);
                    }
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_dialects() {
        let old = "InstanceType=OneSix\nname=My Pack\nnotes=line one\\nline two\n";
        let ini = Ini::parse(old);
        assert_eq!(ini.get("name"), Some("My Pack"));
        assert_eq!(ini.get("notes"), Some("line one\nline two"));

        let qsettings = "[General]\nname=Caf\\xe9 Pack\nEnv=\"{\\\"A\\\": \\\"b\\\"}\"\nOverrideMemory=true\nMaxMemAlloc=4096\n";
        let ini = Ini::parse(qsettings);
        assert_eq!(ini.get("name"), Some("Café Pack"));
        assert_eq!(ini.get("Env"), Some("{\"A\": \"b\"}"));
        assert!(ini.flag("OverrideMemory"));
        assert_eq!(ini.number::<u32>("MaxMemAlloc"), Some(4096));
    }

    #[test]
    fn blanks_and_invalid_read_as_missing() {
        let ini = Ini::parse("JvmArgs=\nJavaPath=@Invalid()\n");
        assert_eq!(ini.get("JvmArgs"), None);
        assert_eq!(ini.get("JavaPath"), None);
        assert!(!ini.flag("Missing"));
    }
}
