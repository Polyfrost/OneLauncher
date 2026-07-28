mod analytics;
mod error;
mod launch;
mod log_replay;
mod process;
mod reattach;
mod session;
mod shared_dir;
mod tail;

pub use analytics::{
    Analytics, DayPlaytime, Persona, PlaytimeStats, ServerStat, WEEKDAY_LABELS, aggregate_servers,
    cluster_analytics, global_analytics,
};
// Minecraft content lives in `oneclient_mc` now; re-exported so the rest of the
// launcher keeps addressing it through `game::`.
pub use oneclient_mc::{
    append_profile_game_arguments, classpaths, download_to_path, fetch_bytes_verified,
    get_classpath_library, get_library, java_arguments, main_class, minecraft_arguments,
    processor_arguments,
};
pub use error::GameError;
pub use launch::{LaunchedGame, is_running, launch_cluster};
pub use process::{
    GameProcess, GameProcessManager, is_process_alive, kill_process, process_start_time,
};
pub use reattach::recover_sessions;
pub use oneclient_mc::{
    DownloadPlan, download_minecraft, download_version_info, get_game_versions,
    get_loader_version, get_loader_versions, get_loaders_for_version, is_version_updated,
    libraries_missing, plan_downloads, resolve_minecraft_version, validate_rules,
};
pub use shared_dir::{
    clear_shared_content, import_manual_content, link_cluster_logs, sync_shared_content,
    unlink_cluster_logs, write_allowed_symlinks,
};
