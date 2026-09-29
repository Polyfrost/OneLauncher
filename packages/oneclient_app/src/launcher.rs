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
    LAUNCHER.get().cloned().ok_or(LauncherError::NotInitialized)
}

pub async fn off_ui<T: Send + 'static>(work: impl Future<Output = T> + Send + 'static) -> T {
    match tokio::spawn(work).await {
        Ok(value) => value,
        Err(err) => std::panic::resume_unwind(err.into_panic()),
    }
}

pub async fn off_ui_blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => value,
        Err(err) => std::panic::resume_unwind(err.into_panic()),
    }
}
