//! The app's handle on the launcher core.
//!
//! The core keeps no process-wide `OnceCell`; a library holding ambient state
//! cannot be constructed twice, or once in a test. The handle lives here
//! instead, in the executable, which is the one place a global is defensible:
//! this *is* the composition root.
//!
//! It exists because `freya::query`'s `QueryCapability::run` and
//! `MutationCapability::run` take only their keys, with no context and no
//! hooks, so there is nowhere else to thread the state through.

use std::sync::{Arc, OnceLock};

use oneclient_core::{LauncherError, LauncherResult, LauncherState};

static LAUNCHER: OnceLock<Arc<LauncherState>> = OnceLock::new();

/// Installs the launcher handle. Called once, from the startup path.
///
/// Returns the existing handle if one was already installed, rather than
/// failing: a second call is a bug in startup ordering, not something a user
/// can trigger, and silently keeping the first is safer than panicking.
pub fn install(state: Arc<LauncherState>) -> Arc<LauncherState> {
    if let Err(existing) = LAUNCHER.set(Arc::clone(&state)) {
        tracing::warn!("launcher state was already installed; keeping the first");
        return existing;
    }
    state
}

/// The launcher handle, or [`LauncherError::NotInitialized`] before startup has
/// finished installing it.
pub fn state() -> LauncherResult<Arc<LauncherState>> {
    LAUNCHER
        .get()
        .cloned()
        .ok_or(LauncherError::NotInitialized)
}
