use std::path::Path;

pub const FILE: &str = "server.properties";

/// Keeps every line as written so comments and unknown keys survive a save
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Properties {
    lines: Vec<String>,
    latin1: bool,
}

impl Properties {
    #[must_use]
    pub fn parse(text: &str) -> Self {
        Self {
            lines: text.lines().map(str::to_string).collect(),
            latin1: false,
        }
    }

    pub fn replace_text(&mut self, text: &str) {
        self.lines = text.lines().map(str::to_string).collect();
    }

    pub async fn read(dir: &Path) -> Self {
        Self::load(dir).await.unwrap_or_default()
    }

    pub async fn load(dir: &Path) -> std::io::Result<Self> {
        match tokio::fs::read(dir.join(FILE)).await {
            Ok(bytes) => Ok(match String::from_utf8(bytes) {
                Ok(text) => Self::parse(&text),
                Err(err) => {
                    let text: String = err.into_bytes().into_iter().map(char::from).collect();
                    Self {
                        latin1: true,
                        ..Self::parse(&text)
                    }
                }
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err),
        }
    }

    pub async fn write(&self, dir: &Path) -> std::io::Result<()> {
        let text = self.to_string();
        let bytes = if self.latin1 {
            text.chars()
                .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
                .collect()
        } else {
            text.into_bytes()
        };
        polyio::write_atomic(dir.join(FILE), bytes)
            .await
            .map_err(std::io::Error::other)
    }

    fn split(line: &str) -> Option<(&str, &str)> {
        const BLANK: [char; 3] = [' ', '\t', '\x0c'];
        let line = line.trim_start_matches(BLANK);
        if line.is_empty() || line.starts_with(['#', '!']) {
            return None;
        }
        let mut escaped = false;
        let end = line
            .char_indices()
            .find(|&(_, c)| {
                let hit = !escaped && matches!(c, '=' | ':' | ' ' | '\t' | '\x0c');
                escaped = !escaped && c == '\\';
                hit
            })
            .map_or(line.len(), |(i, _)| i);
        let rest = line[end..].trim_start_matches(BLANK);
        let rest = rest
            .strip_prefix(['=', ':'])
            .unwrap_or(rest)
            .trim_start_matches(BLANK);
        Some((&line[..end], rest))
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<String> {
        self.lines
            .iter()
            .filter_map(|line| Self::split(line))
            .rev()
            .find(|(k, _)| *k == key)
            .map(|(_, value)| unescape(value))
    }

    #[must_use]
    pub fn get_or(&self, key: &str, fallback: &str) -> String {
        self.get(key)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let line = format!("{key}={}", escape(value));
        match self
            .lines
            .iter()
            .rposition(|l| Self::split(l).is_some_and(|(k, _)| k == key))
        {
            Some(index) => self.lines[index] = line,
            None => self.lines.push(line),
        }
    }
}

impl std::fmt::Display for Properties {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.lines {
            writeln!(f, "{line}")?;
        }
        Ok(())
    }
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for (i, ch) in value.chars().enumerate() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x0c' => out.push_str("\\f"),
            ' ' if i == 0 => out.push_str("\\ "),
            ':' | '=' => {
                out.push('\\');
                out.push(ch);
            }
            c if !c.is_ascii() => {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut units: Vec<u16> = Vec::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'u') {
            chars.next();
            let hex: String = chars.by_ref().take(4).collect();
            if let Ok(unit) = u16::from_str_radix(&hex, 16) {
                units.push(unit);
            }
            continue;
        }
        out.extend(char::decode_utf16(units.drain(..)).filter_map(Result::ok));
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\x0c'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out.extend(char::decode_utf16(units).filter_map(Result::ok));
    out
}

#[cfg(test)]
mod tests {
    use super::Properties;

    #[test]
    fn set_keeps_comments_and_round_trips_escapes() {
        let mut props =
            Properties::parse("#Minecraft server properties\nmotd=A Minecraft Server\npvp=true\n");
        props.set("pvp", "false");
        props.set("motd", "Hi: ever SMP §a✓");
        props.set("server-port", "25566");

        assert_eq!(props.get("pvp").as_deref(), Some("false"));
        assert_eq!(props.get("motd").as_deref(), Some("Hi: ever SMP §a✓"));
        assert_eq!(props.get("server-port").as_deref(), Some("25566"));
        assert!(
            props
                .to_string()
                .starts_with("#Minecraft server properties\n")
        );
        assert_eq!(Properties::parse(&props.to_string()), props);
    }

    #[test]
    fn characters_outside_the_bmp_are_written_as_surrogate_pairs() {
        let mut props = Properties::default();
        props.set("motd", "hi \u{1f600}!");
        assert_eq!(props.to_string(), "motd=hi \\ud83d\\ude00!\n");
        assert_eq!(props.get("motd").as_deref(), Some("hi \u{1f600}!"));
    }

    #[test]
    fn keys_follow_java_separators_and_the_last_one_wins() {
        let mut props = Properties::parse(
            "# c\n! c\npvp=true\nmotd : hello\ndifficulty hard\n  level-name\t=  world\npvp = false\nempty\n",
        );
        assert_eq!(props.get("pvp").as_deref(), Some("false"));
        assert_eq!(props.get("motd").as_deref(), Some("hello"));
        assert_eq!(props.get("difficulty").as_deref(), Some("hard"));
        assert_eq!(props.get("level-name").as_deref(), Some("world"));
        assert_eq!(props.get("empty").as_deref(), Some(""));
        assert_eq!(props.get("# c"), None);
        props.set("pvp", "true");
        assert!(props.to_string().starts_with("# c\n! c\npvp=true\n"));
        assert!(props.to_string().ends_with("pvp=true\nempty\n"));
        assert_eq!(props.get("pvp").as_deref(), Some("true"));
    }

    #[test]
    fn control_characters_and_leading_spaces_are_escaped() {
        let mut props = Properties::default();
        props.set("motd", "  a\tb\r\nc");
        assert_eq!(props.to_string(), "motd=\\  a\\tb\\r\\nc\n");
        assert_eq!(props.get("motd").as_deref(), Some("  a\tb\r\nc"));
    }

    #[tokio::test]
    async fn latin1_files_are_saved_back_as_latin1() {
        let tmp = polyio::tempdir().await.expect("dir");
        let dir = tmp.dir_path();
        tokio::fs::write(dir.join(super::FILE), b"motd=Caf\xe9\npvp=true\n")
            .await
            .unwrap();
        let mut props = Properties::load(dir).await.unwrap();
        assert_eq!(props.get("motd").as_deref(), Some("Caf\u{e9}"));
        props.set("pvp", "false");
        props.write(dir).await.unwrap();
        assert_eq!(
            tokio::fs::read(dir.join(super::FILE)).await.unwrap(),
            b"motd=Caf\xe9\npvp=false\n"
        );
    }

    #[tokio::test]
    async fn replacing_the_text_keeps_a_latin1_file_latin1() {
        let tmp = polyio::tempdir().await.expect("dir");
        let dir = tmp.dir_path();
        tokio::fs::write(dir.join(super::FILE), b"motd=Caf\xe9\n")
            .await
            .unwrap();
        let mut props = Properties::load(dir).await.unwrap();
        props.replace_text("#edited\nmotd=Caf\u{e9} \u{e8}\n");
        props.write(dir).await.unwrap();
        assert_eq!(
            tokio::fs::read(dir.join(super::FILE)).await.unwrap(),
            b"#edited\nmotd=Caf\xe9 \xe8\n"
        );
    }
}
