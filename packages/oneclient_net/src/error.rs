/// Renders an error together with every `source()` beneath it.
///
/// `reqwest::Error`'s own `Display` stops at "error sending request for url
/// (...)". The fact that actually names the failure — a rustls handshake
/// alert, an untrusted certificate, a DNS lookup failure, `ECONNREFUSED` —
/// lives further down the chain, so without this every transport failure in a
/// user report reads identically and is undiagnosable.
#[must_use]
pub fn error_chain(err: &dyn std::error::Error) -> String {
    let mut rendered = err.to_string();
    let mut cursor = err.source();

    while let Some(cause) = cursor {
        let text = cause.to_string();
        // hyper and reqwest repeat the same sentence across several layers.
        if !rendered.ends_with(&text) {
            rendered.push_str(": ");
            rendered.push_str(&text);
        }
        cursor = cause.source();
    }

    rendered
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("{}", error_chain(.0))]
    ReqwestError(#[from] reqwest::Error),

    #[error("IO Error: {0}")]
    IOError(#[from] polyio::IOError),

    #[error(
        "Failed to parse {type_name} from {url} (HTTP {status}): {source}; body starts: {snippet}"
    )]
    DeserializeError {
        #[source]
        source: serde_json::Error,
        type_name: String,
        url: String,
        status: u16,
        snippet: String,
    },

    #[error("HTTP {status} from {url}: {snippet}")]
    HttpStatus {
        status: u16,
        url: String,
        snippet: String,
    },

    #[error("Failed to serialize request body: {0}")]
    SerializeError(#[source] serde_json::Error),

    /// A download's contents did not match the hash the manifest promised.
    #[error("{source_desc} has SHA-1 {actual}, expected {expected}")]
    HashMismatch {
        source_desc: String,
        expected: String,
        actual: String,
    },

    #[error("Invalid URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Invalid HTTP header name: {0}")]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    #[error("Invalid HTTP header value: {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
}

impl RequestError {
    /// Whether this is the user's environment rather than a launcher bug: offline,
    /// connection refused, timed out, disk full.
    ///
    /// The transport reports the fact; deciding that such errors stay out of
    /// Sentry is a policy the composition layer applies on top.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::ReqwestError(source) => source.is_timeout() || source.is_connect(),
            Self::IOError(source) => is_transient_io(source),
            _ => false,
        }
    }
}

fn is_transient_io(error: &polyio::IOError) -> bool {
    let source = match error {
        polyio::IOError::IOError(source) | polyio::IOError::PathIOError { source, .. } => source,
        _ => return false,
    };

    use std::io::ErrorKind;

    // Out of disk space. `ErrorKind::StorageFull` is still unstable, so match the
    // raw OS codes. They are platform-gated so a Unix errno cannot collide with a
    // Windows code (112 is the unrelated EHOSTDOWN on Linux).
    if let Some(code) = source.raw_os_error() {
        #[cfg(unix)]
        if code == 28 {
            // ENOSPC
            return true;
        }
        #[cfg(windows)]
        if code == 112 || code == 39 {
            // ERROR_DISK_FULL / ERROR_HANDLE_DISK_FULL
            return true;
        }
        let _ = code;
    }

    matches!(
        source.kind(),
        ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::NotConnected
            | ErrorKind::TimedOut
    )
}

pub(crate) fn body_snippet(bytes: &[u8]) -> String {
    const MAX: usize = 240;
    if bytes.is_empty() {
        return "<empty body>".to_string();
    }
    let text = String::from_utf8_lossy(bytes);
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return "<non-text body>".to_string();
    }
    if collapsed.len() > MAX {
        let end = collapsed
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|i| *i <= MAX)
            .last()
            .unwrap_or(0);
        format!("{}...", &collapsed[..end])
    } else {
        collapsed
    }
}
