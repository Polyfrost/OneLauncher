//! HTTP transport for OneClient: one client with retries, throttling and a
//! global in-flight cap, plus the two things every caller was reimplementing:
//! ETag-cached fetches and hash-verified downloads.
//!
//! Endpoints and credentials arrive as data ([`NetConfig`]) rather than being
//! looked up, so the transport does not depend on the launcher state that owns
//! the client.

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
pub use error::{RequestError, error_chain};
pub use request::*;
pub use response::*;
pub use service::RequestClient;
