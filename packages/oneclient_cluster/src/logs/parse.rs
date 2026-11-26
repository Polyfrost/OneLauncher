use std::borrow::Cow;
use std::sync::{LazyLock, RwLock};

use regex::Regex;

use super::LogLevel;

static SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(--accessToken|--session(?:Id)?)['"]?([\s,=]+)['"]?[^\s,\[\]'"]+|(\(Session ID is )[^)\s]+|(token:)[^:\s]+|eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*"#,
    )
    .expect("secret pattern")
});

static NAMES: LazyLock<RwLock<Vec<String>>> = LazyLock::new(|| {
    let home_user = std::env::home_dir()
        .and_then(|home| home.file_name()?.to_str().map(str::to_string))
        .filter(|name| name.len() >= 3);
    RwLock::new(home_user.into_iter().collect())
});

pub fn censor_username(name: &str) {
    let name = name.trim();
    if name.len() < 3 {
        return;
    }
    let mut names = NAMES.write().unwrap_or_else(|err| err.into_inner());
    if !names.iter().any(|known| known == name) {
        names.push(name.to_string());
    }
}

pub fn censor(line: &str) -> Cow<'_, str> {
    let out = SECRET.replace_all(line, "${1}${2}${3}${4}[censored]");
    let names = NAMES.read().unwrap_or_else(|err| err.into_inner());
    censor_names(out, &names)
}

fn censor_names<'a>(mut text: Cow<'a, str>, names: &[String]) -> Cow<'a, str> {
    for name in names {
        if text.contains(name.as_str()) {
            text = Cow::Owned(text.replace(name.as_str(), "[censored]"));
        }
    }
    text
}

pub fn parse_level(line: &str) -> Option<LogLevel> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'['
            && let Some(rel_end) = line[i + 1..].find(']')
        {
            let seg = &line[i + 1..i + 1 + rel_end];
            if let Some(level) = level_from_segment(seg) {
                return Some(level);
            }
            i = i + 1 + rel_end + 1;
            continue;
        }
        i += 1;
    }
    None
}

fn level_from_segment(seg: &str) -> Option<LogLevel> {
    let token = seg.rsplit('/').next().unwrap_or(seg).trim();
    LogLevel::from_token(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_log_lines_stay_borrowed() {
        let line = "[Render thread/INFO]: Loaded 138 advancements";
        let names = [String::from("SamplePlayer")];

        assert!(matches!(
            censor_names(Cow::Borrowed(line), &names),
            Cow::Borrowed(text) if text == line
        ));
    }

    #[test]
    fn redacts_names_in_registration_order_after_secrets() {
        let line = "--accessToken SYNTHETIC EchoGuest EchoGuest123 EchoGuest";
        let redacted = SECRET.replace_all(line, "${1}${2}${3}${4}[censored]");
        let names = [String::from("EchoGuest"), String::from("EchoGuest123")];

        assert_eq!(
            censor_names(redacted, &names),
            "--accessToken [censored] [censored] [censored]123 [censored]"
        );
    }
}

#[cfg(test)]
mod quoted_tests {
    use super::SECRET;

    fn redact(line: &str) -> String {
        SECRET
            .replace_all(line, "${1}${2}${3}${4}[censored]")
            .into_owned()
    }

    #[test]
    fn a_quoted_access_token_is_censored() {
        // The launch header quotes every argument on Windows, and an access token
        // is not always jwt shaped, so the quotes cannot be what hides it
        for line in [
            "'--accessToken' 'abc123OPAQUE' --uuid '1234'",
            "\"--accessToken\" \"abc123OPAQUE\"",
            "--accessToken abc123OPAQUE",
        ] {
            assert!(!redact(line).contains("abc123OPAQUE"), "{line}");
        }
    }

    #[test]
    fn a_quoted_session_token_is_censored() {
        assert!(!redact("'--session' 'abc123OPAQUE'").contains("abc123OPAQUE"));
    }
}
