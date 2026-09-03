//! Leaf crate must never depend on the database network stack or any
//! launcher subsystem or the dependency graph cycles back into a monolith

pub mod consent;
pub mod constants;
pub mod domain;
pub mod memory;
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
pub use memory::{MEMORY_HEADROOM_GB, default_mem_max, default_mem_max_for_total, total_ram_mb};
pub use os_ext::OsExt;
pub use patch::Patch;
pub use search::{MatchScore, SearchQuery, normalize_query};
pub use version::{ParsedMcVersion, VersionKey, format_mc_version, parse_mc_version};
