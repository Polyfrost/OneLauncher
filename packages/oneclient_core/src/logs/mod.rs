mod error;
mod manage;
mod mclogs;
mod parse;

use std::path::PathBuf;

use chrono::{DateTime, Utc};

pub use error::LogsError;
pub use manage::{delete_log_at, list_cluster_logs, read_log_at};
pub use mclogs::upload_log_at;
pub use parse::parse_level;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogKind {
    Game { cluster_id: i64 },
    Minecraft,
    CrashReport,
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogFileInfo {
    pub name: String,
    pub kind: LogKind,
    pub size_bytes: u64,
    pub modified: DateTime<Utc>,
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Unknown,
}

impl LogLevel {
    pub fn from_token(token: &str) -> Option<Self> {
        match token.to_ascii_uppercase().as_str() {
            "TRACE" => Some(Self::Trace),
            "DEBUG" => Some(Self::Debug),
            "INFO" => Some(Self::Info),
            "WARN" | "WARNING" => Some(Self::Warn),
            "ERROR" => Some(Self::Error),
            "FATAL" => Some(Self::Fatal),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Fatal => "FATAL",
            Self::Unknown => "",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    pub number: usize,
    pub level: LogLevel,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ReadOptions {
    pub level_filter: Option<LogLevel>,
    pub search: Option<String>,
    pub max_lines: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MclogsUploadResponse {
    pub id: String,
    pub url: String,
    pub raw: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bracket_levels() {
        assert_eq!(
            parse_level("[12:00:00] [Render thread/INFO]: hello"),
            Some(LogLevel::Info)
        );
        assert_eq!(
            parse_level("[12:00:00] [Server thread/WARN]: uh oh"),
            Some(LogLevel::Warn)
        );
        assert_eq!(parse_level("[ERROR] boom"), Some(LogLevel::Error));
        assert_eq!(parse_level("[main/FATAL]"), Some(LogLevel::Fatal));
        
        assert_eq!(parse_level("\tat java.base/java.lang.Thread.run"), None);
        assert_eq!(parse_level("[12:34:56] just a timestamp"), None);
    }

    #[test]
    fn carries_level_forward() {
        let opts = ReadOptions::default();

        let lines = super::manage::lines_from(
            "[12:00:00] [Server thread/ERROR]: boom\n\tat Foo.bar\nplain line",
            &opts,
        );

        assert_eq!(lines[0].level, LogLevel::Error);
        assert_eq!(lines[1].level, LogLevel::Error);
        assert_eq!(lines[2].level, LogLevel::Error);
    }
}
