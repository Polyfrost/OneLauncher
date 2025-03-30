//! **`OneLauncher` Core Utilities**
//! Standard asynchronous utilities and wrappers for use within the launcher core.
//!
//! - [`http`]: Async extensions and wrappers around [`reqwest`] functions.
//! - [`java`]: Async utilities for managing and downloading Java versions.
//! - [`watcher`]: Async utilities for watching files with [`notify`].

pub mod http;
pub mod java;
pub mod pagination;
pub mod watcher;
pub mod crypto;

#[cfg(feature = "tauri")]
pub mod window;
