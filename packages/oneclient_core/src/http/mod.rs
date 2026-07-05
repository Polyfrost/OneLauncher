mod request;
mod response;
mod service;

pub use request::*;
pub use response::*;
pub use service::*;

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error("IO Error: {0}")]
    IOError(#[from] polyio::IOError),

    #[error(
        "Failed to parse {type_name} from {url} (HTTP {status}): {source} — body starts: {snippet}"
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

    #[error("Invalid URL: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Invalid HTTP header name: {0}")]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    #[error("Invalid HTTP header value: {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
}