//! The global lives here not in the core so the library stays constructible in tests
//! It is needed because `freya::query`'s `run` methods take only keys with no context

use std::sync::{Arc, OnceLock};

use oneclient_core::{LauncherError, LauncherResult, LauncherState};

static LAUNCHER: OnceLock<Arc<LauncherState>> = OnceLock::new();

/// Returns the existing handle on a second call rather than panicking
pub fn install(state: Arc<LauncherState>) -> Arc<LauncherState> {
    if let Err(existing) = LAUNCHER.set(Arc::clone(&state)) {
        tracing::warn!("launcher state was already installed; keeping the first");
        return existing;
    }
    state
}

/// Errors with [`LauncherError::NotInitialized`] before startup installs the handle
pub fn state() -> LauncherResult<Arc<LauncherState>> {
    LAUNCHER
        .get()
        .cloned()
        .ok_or(LauncherError::NotInitialized)
}
