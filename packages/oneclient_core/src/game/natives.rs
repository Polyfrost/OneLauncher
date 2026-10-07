use std::path::PathBuf;

use oneclient_common::domain::GameLoader;
use oneclient_common::paths;
use oneclient_mc::MetadataStore;

use crate::clusters::Cluster;
use crate::game::NativesMode;
use crate::state::LauncherState;

pub(crate) fn natives_dir(version_name: &str) -> Option<PathBuf> {
    paths::natives_dir().ok().map(|dir| dir.join(version_name))
}

pub(crate) fn cached_version_name(metadata: &MetadataStore, cluster: &Cluster) -> Option<String> {
    let mc_version = oneclient_common::version::normalize_mc_version_input(&cluster.mc_version);

    if cluster.mc_loader == GameLoader::Vanilla {
        return Some(mc_version);
    }

    let loader = oneclient_mc::get_loader_version_cached(
        metadata,
        &mc_version,
        cluster.mc_loader,
        cluster.mc_loader_version.as_deref(),
    )
    .ok()??;

    Some(format!("{mc_version}-{}", loader.id))
}

pub(crate) fn natives_holder(
    state: &LauncherState,
    version_name: &str,
    exclude: Option<i64>,
) -> Option<i64> {
    let dir = natives_dir(version_name)?;
    state.games.natives_in_use_by(&dir, exclude)
}

pub(crate) fn natives_mode(
    state: &LauncherState,
    version_name: &str,
    exclude: Option<i64>,
) -> NativesMode {
    match natives_holder(state, version_name, exclude) {
        Some(holder) => {
            tracing::info!(
                version_name,
                holder,
                "natives are loaded by a running game; leaving them in place"
            );
            NativesMode::LeaveInPlace
        }
        None => NativesMode::Extract,
    }
}
