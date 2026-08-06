//! The vocabulary every other OneClient crate is allowed to depend on.
//!
//! This crate is a leaf: it knows about the filesystem layout, the target
//! platform, Minecraft version strings and the content-type taxonomy, and
//! nothing else. It must never gain a dependency on the database, the network
//! stack, or any launcher subsystem. Everything above it depends on it, so a
//! back-edge here would recreate the monolith the split is undoing.

pub mod constants;
pub mod domain;
pub mod os_ext;
pub mod paths;
pub mod patch;
pub mod search;
pub mod version;

mod error;

pub use domain::{
    ContentType, GameLoader, HashAlgorithm, PackageUpdateMode, ProviderId, Resolution,
};
pub use error::{PathsError, PathsResult};
pub use os_ext::OsExt;
pub use patch::Patch;
pub use search::{MatchScore, SearchQuery, normalize_query};
pub use version::{ParsedMcVersion, VersionKey, format_mc_version, parse_mc_version};
