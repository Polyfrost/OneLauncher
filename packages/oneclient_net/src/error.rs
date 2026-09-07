/// `reqwest::Error`'s own `Display` stops at "error sending request for url"
/// the fact that names the failure lives further down the `source()` chain
#[must_use]
pub fn error_chain(err: &dyn std::error::Error) -> String {
    let mut rendered = err.to_string();
    let mut cursor = err.source();

    while let Some(cause) = cursor {
        let text = cause.to_string();
        // hyper and reqwest repeat the same sentence across several layers
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

    #[error("{source_desc} has SHA-1 {actual}, expected {expected}")]
    HashMismatch {
        source_desc: String,
        expected: String,
        actual: String,
    },

    /// Only reachable when no hash was supplied otherwise it fails as a mismatch
    #[error("{source_desc} ended after {actual} of {expected} bytes")]
    IncompleteBody {
        source_desc: String,
        expected: u64,
        actual: u64,
    },

    #[error(
        "OneClient did not contact {url} because the Terms of Service and Privacy Policy were \
         declined. Accept them in Settings to turn Polyfrost services back on."
    )]
    ConsentRequired { url: String },

    #[error("Invalid URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Invalid HTTP header name: {0}")]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    #[error("Invalid HTTP header value: {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkFailure {
    Certificate,
	Handshake,
    Dns,
    Generic,
}

impl NetworkFailure {
    #[must_use]
    pub fn user_message(self) -> &'static str {
        match self {
            Self::Certificate => {
                "Something on this network or machine is intercepting secure connections, so OneClient cannot verify it is really talking to the servers it needs. This is usually antivirus HTTPS scanning, a school or workplace filter, or a VPN. Try another network, turn off HTTPS/SSL scanning in your antivirus, or ask your network administrator to allow OneClient."
            }
            Self::Handshake => {
                "OneClient reached the server but the two could not agree on a secure connection. A proxy, filter, or antivirus in between is usually using outdated encryption settings. Try another network or turn off HTTPS/SSL scanning in your antivirus."
            }
            Self::Dns => {
                "OneClient could not look up the address of the server. Check your internet connection, disable any VPN or proxy, and try a different DNS server such as 1.1.1.1."
            }
            Self::Generic => {
                "OneClient could not reach the server. Check your internet connection, and any firewall, proxy, or VPN that could be blocking it."
            }
        }
    }

    /// Retrying or waiting cannot clear these the user has to change something
    #[must_use]
    pub fn is_tls_interception(self) -> bool {
        matches!(self, Self::Certificate | Self::Handshake)
    }
}

#[must_use]
pub fn classify_network_failure(err: &reqwest::Error) -> NetworkFailure {
    let chain = error_chain(err).to_ascii_lowercase();

    // Certificate before handshake a rejected certificate also aborts the
    // handshake and the certificate wording is the more specific of the two
    const CERTIFICATE: &[&str] = &[
        "invalid peer certificate",
        "unknownissuer",
        "certificate",
        "certexpired",
        "notvalidforname",
    ];
    const HANDSHAKE: &[&str] = &[
        "handshake",
        "received fatal alert",
        "peer misbehaved",
        "no cipher suites in common",
        "protocol version",
        "unsupported protocol",
    ];
    const DNS: &[&str] = &[
        "dns error",
        "failed to lookup address",
        "no record found",
        "nodename nor servname",
        "name or service not known",
        "no such host",
    ];

    let matches = |markers: &[&str]| markers.iter().any(|marker| chain.contains(marker));

    if matches(CERTIFICATE) {
        NetworkFailure::Certificate
    } else if matches(HANDSHAKE) {
        NetworkFailure::Handshake
    } else if matches(DNS) {
        NetworkFailure::Dns
    } else {
        NetworkFailure::Generic
    }
}

impl RequestError {
    /// Whether this is the user's environment rather than a launcher bug offline
    /// connection refused timed out disk full intercepted TLS
    #[must_use]
    pub fn is_transient(&self) -> bool {
        match self {
            Self::ReqwestError(source) => {
                source.is_timeout()
                    || source.is_connect()
                    || classify_network_failure(source).is_tls_interception()
            }
            Self::IOError(source) => is_transient_io(source),
            _ => false,
        }
    }

    #[must_use]
    pub fn network_failure(&self) -> Option<NetworkFailure> {
        let Self::ReqwestError(source) = self else {
            return None;
        };

        let failure = classify_network_failure(source);

        if failure != NetworkFailure::Generic || source.is_connect() || source.is_timeout() {
            Some(failure)
        } else {
            None
        }
    }

    #[must_use]
    pub fn user_message(&self) -> Option<&'static str> {
        self.network_failure().map(NetworkFailure::user_message)
    }
}

fn is_transient_io(error: &polyio::IOError) -> bool {
    let source = match error {
        polyio::IOError::IOError(source) | polyio::IOError::PathIOError { source, .. } => source,
        _ => return false,
    };

    use std::io::ErrorKind;

    // `ErrorKind::StorageFull` is unstable so match raw OS codes platform-gated
    // because 112 is the unrelated EHOSTDOWN on Linux
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
