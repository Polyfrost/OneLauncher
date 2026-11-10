pub const DUMMY_REPLACE_NEWLINE: &str = "\n";

/// The Microsoft sign-in app. Set `REMIXCLIENT_MICROSOFT_CLIENT_ID` to your
/// own Azure application before distributing a public release; the fallback
/// keeps the upstream app id so local builds can still sign in.
pub const MICROSOFT_CLIENT_ID: &str = match option_env!("REMIXCLIENT_MICROSOFT_CLIENT_ID") {
    Some(id) => id,
    None => "9419b7ee-1448-4d1b-b52a-550d8f36ab56",
};
pub const MINECRAFT_SCOPES: &str = "XboxLive.SignIn XboxLive.offline_access";

/// CurseForge's public API key. Override with
/// `REMIXCLIENT_CURSEFORGE_API_KEY` when distributing your own build.
pub const CURSEFORGE_API_KEY: &str = match option_env!("REMIXCLIENT_CURSEFORGE_API_KEY") {
    Some(key) => key,
    None => "$2a$10$6utA1UNSmFPrE/Lh7b7ndeeGmiOkjKNY8kpFB0fsmE/d42ZAfFgCe",
};
pub const DISCORD_CLIENT_ID: &str = match option_env!("REMIXCLIENT_DISCORD_CLIENT_ID") {
    Some(id) => id,
    None => "1569698419491217500",
};

pub const MODRINTH_API_URL: &str = "https://api.modrinth.com";
pub const MODRINTH_CDN_PREFIX: &str = "https://cdn.modrinth.com/data/";
pub const CURSEFORGE_API_URL: &str = "https://api.curseforge.com/v1";
pub const CURSEFORGE_GAME_ID: u32 = 432;
pub const METADATA_API_URL: &str = "https://meta.polyfrost.org";
pub const MCLOGS_API_URL: &str = "https://api.mclo.gs/1";
pub const SKYCLIENT_BASE_URL: &str =
    "https://raw.githubusercontent.com/SkyblockClient/SkyblockClient-REPO/refs/heads/main/v1";
pub const META_URL_BASE: &str = "https://data-v2.polyfrost.org";
pub const TOS_URL: &str =
    "https://github.com/RemixClient/RemixClient/blob/main/docs/terms.md";
pub const PRIVACY_URL: &str =
    "https://github.com/RemixClient/RemixClient/blob/main/docs/privacy.md";
pub const PLUS_BACKEND_URL: &str = "https://plus.polyfrost.org";
pub const CRASH_DATA_URL: &str =
    "https://raw.githubusercontent.com/Polyfrost/CrashData/refs/heads/main/crashes.json";

pub const DISCORD_URL: &str = "https://discord.com/invite/polyfrost";

// Crash reports are opt-in and go to the maintainer's own project only. A
// build without `REMIXCLIENT_SENTRY_DSN` sends nothing, so a fork never writes
// into the upstream project's telemetry.
pub const SENTRY_DSN: &str = match option_env!("REMIXCLIENT_SENTRY_DSN") {
    Some(dsn) => dsn,
    None => "",
};

pub const TARGET_OS: &str = cfg_select! {
    target_os = "windows" => "windows",
    target_os = "macos" => "osx",
    target_os = "linux" => "linux"
};

pub const NATIVE_ARCH: &str = cfg_select! {
    target_arch = "x86" => "32",
    target_arch = "x86_64" => "64",
    _ => "64"
};

pub const JAVA_BIN: &str = if cfg!(windows) { "javaw.exe" } else { "java" };

pub const ARCH_WIDTH: &str = if cfg!(target_pointer_width = "64") {
    "64"
} else {
    "32"
};

pub const LINE_ENDING: &str = if cfg!(windows) { "\r\n" } else { "\n" };
pub const CLASSPATH_SEPARATOR: &str = if cfg!(windows) { ";" } else { ":" };
