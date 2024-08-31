//! **OneLauncher Utilities**
//! Standard asynchronous utilities and wrappers for use in the launcher.
//!
//! - [`io`]: Async wrapper around [`tokio::fs`] and [`std::io::Error`] for our error system.
//! - [`http`]: Async extensions and wrappers around [`reqwest`] functions.
//! - [`java`]: Async utilities for managing and downloading Java versions.
//! - [`platform`]: Async utilities for managing OS-specific [`interpulse`] operations and rules.
//! - [`watcher`]: Async utilities for watching files with [`notify`].
//! - [`logging`]: Async utilities for log4j parsing with [`nom`].

pub mod http;
pub mod io;
pub mod java;
pub mod logging;
pub mod platform;
pub mod watcher;

/// Simple macro that takes a mutable reference and inserts it into a codeblock closure
/// as an owned reference.
///
/// mutable reference gets epically owned by free thinking macro!!!!! (not clickbait)
/// im going insane insane insane insane insane insane insane insane insane
macro_rules! ref_owned {
	($id:ident = $init:expr => $transform:block) => {{
		let mut it = $init;
		{
			let $id = &mut it;
			$transform;
		}
		it
	}};
}
