//! Everything the user installs into a cluster: mods and resource packs from
//! Modrinth and CurseForge, the content-addressed artifact store that holds
//! them, and the Polyfrost bundles that curate sets of them.
//!
//! `packages` and `bundles` ship together because they are one concern viewed
//! from two sides: the store holds files, bundles decide which files a cluster
//! should have. The dependency between them runs one way only, though;
//! `bundles` builds on `packages`, never the reverse.

pub mod bundles;
pub mod packages;

mod ctx;
mod error;

pub use ctx::ContentCtx;
pub use error::{ContentError, ContentResult};
