mod analytics;
mod error;
pub mod fabric;
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
pub use oneclient_mc::{
    append_profile_game_arguments, classpaths, download_to_path, fetch_bytes_verified,
    get_classpath_library, get_library, java_arguments, main_class, minecraft_arguments,
    processor_arguments,
};
pub mod diagnosis;

pub use diagnosis::{CrashDiagnosis, diagnose};
pub use error::GameError;
pub use launch::{LaunchedGame, is_running, launch_cluster, offer_repair};
pub use process::{
    GameProcess, GameProcessManager, is_process_alive, kill_process, process_start_time,
};
pub use reattach::recover_sessions;
pub use oneclient_mc::{
    DownloadPlan, confirm_incomplete_install, download_minecraft, download_version_info, get_game_versions,
    get_loader_version, get_loader_versions, get_loaders_for_version, is_version_updated,
    game_files_missing, libraries_missing, plan_downloads, resolve_minecraft_version,
    validate_rules,
    verify_game_files,
};
pub use fabric::{mods_folder_argument, uses_cluster_mods_folder};
pub use shared_dir::{
    dematerialize_content, import_manual_content, link_cluster_logs, materialize_content,
    unlink_cluster_logs, write_allowed_symlinks,
};
