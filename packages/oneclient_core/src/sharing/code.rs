use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const CODE_LENGTH: usize = 8;
pub const SHARE_LINK_BASE: &str = "https://polyfrost.org/share/";
pub const SHARE_LINK_HOSTS: &[&str] =
    &["polyfrost.org", "www.polyfrost.org", "share.polyfrost.org"];
pub const APP_LINK_HOST: &str = "share";

const APP_SCHEMES: &[&str] = &["oneclient", "oneclient-dev"];
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShareCode(String);

impl ShareCode {
    #[must_use]
    pub fn generate() -> Self {
        let mut bits: u64 = rand::random();
        let code = (0..CODE_LENGTH)
            .map(|_| {
                let index = (bits & 0x1f) as usize;
                bits >>= 5;
                ALPHABET[index] as char
            })
            .collect();
        Self(code)
    }

    #[must_use]
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.contains("://") {
            return Self::from_link(input);
        }
        Self::from_code(input)
    }

    fn from_code(input: &str) -> Option<Self> {
        let mut code = String::with_capacity(CODE_LENGTH);
        for ch in input.chars() {
            if ch == '-' || ch.is_whitespace() {
                continue;
            }
            let normal = match ch.to_ascii_uppercase() {
                'O' => '0',
                'I' | 'L' => '1',
                other => other,
            };
            if !normal.is_ascii() || !ALPHABET.contains(&(normal as u8)) {
                return None;
            }
            code.push(normal);
        }
        (code.len() == CODE_LENGTH).then_some(Self(code))
    }

    fn from_link(input: &str) -> Option<Self> {
        let url = url::Url::parse(input).ok()?;
        let scheme = url.scheme().to_ascii_lowercase();
        let host = url.host_str()?.to_ascii_lowercase();
        let last = url
            .path_segments()?
            .rfind(|segment| !segment.is_empty())?
            .to_string();
        let ours = if APP_SCHEMES.contains(&scheme.as_str()) {
            host == APP_LINK_HOST
        } else {
            scheme == "https" && SHARE_LINK_HOSTS.contains(&host.as_str())
        };
        if !ours {
            return None;
        }
        Self::from_code(&last)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn pretty(&self) -> String {
        let (head, tail) = self.0.split_at(CODE_LENGTH / 2);
        format!("{head}-{tail}")
    }

    #[must_use]
    pub fn web_link(&self) -> String {
        format!("{SHARE_LINK_BASE}{}", self.0)
    }

    #[must_use]
    pub fn app_link(&self, scheme: &str) -> String {
        format!("{scheme}://{APP_LINK_HOST}/{}", self.0)
    }
}

impl fmt::Display for ShareCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.pretty())
    }
}

impl Serialize for ShareCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ShareCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::from_code(&raw).ok_or_else(|| serde::de::Error::custom("not a share code"))
    }
}
