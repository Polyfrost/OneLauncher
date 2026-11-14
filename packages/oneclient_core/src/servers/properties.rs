use std::path::Path;

pub const FILE: &str = "server.properties";

/// Keeps every line as written so comments and unknown keys survive a save
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Properties {
    lines: Vec<String>,
}

impl Properties {
    #[must_use]
    pub fn parse(text: &str) -> Self {
        Self {
            lines: text.lines().map(str::to_string).collect(),
        }
    }

    pub async fn read(dir: &Path) -> Self {
        Self::load(dir).await.unwrap_or_default()
    }

    pub async fn load(dir: &Path) -> std::io::Result<Self> {
        match tokio::fs::read(dir.join(FILE)).await {
            Ok(bytes) => Ok(Self::parse(&decode(bytes))),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err),
        }
    }

    pub async fn write(&self, dir: &Path) -> std::io::Result<()> {
        let tmp = dir.join(format!("{FILE}.tmp"));
        tokio::fs::write(&tmp, self.to_string()).await?;
        tokio::fs::rename(&tmp, dir.join(FILE)).await
    }

    fn split(line: &str) -> Option<(&str, &str)> {
        if line.trim_start().starts_with(['#', '!']) {
            return None;
        }
        line.split_once('=')
            .map(|(key, value)| (key.trim(), value.trim_start()))
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<String> {
        self.lines
            .iter()
            .filter_map(|line| Self::split(line))
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
            .position(|l| Self::split(l).is_some_and(|(k, _)| k == key))
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

fn decode(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|err| err.into_bytes().into_iter().map(char::from).collect())
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            ':' | '=' => {
                out.push('\\');
                out.push(ch);
            }
            c if !c.is_ascii() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
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
}
