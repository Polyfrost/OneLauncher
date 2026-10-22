mod cache;
mod config;
mod download;
mod error;
mod request;
mod response;
mod service;

pub mod status;

pub use cache::{EtagPolicy, Fetched, commit_etag, fetch_cached};
pub use config::NetConfig;
pub use download::{download_verified, fetch_verified, matches_on_disk};
pub use error::{NetworkFailure, RequestError, classify_network_failure, error_chain};
pub use request::*;
pub use response::*;
pub use service::RequestClient;
