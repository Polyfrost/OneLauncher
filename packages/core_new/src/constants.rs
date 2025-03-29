//! **`OneLauncher` Constants**
//!
//! Public constant variables and strings that are used throughout the launcher.

// =========== Core Metadata ===========
/// The name of the launcher.
pub const NAME: &str = match option_env!("LAUNCHER_NAME") {
	Some(name) => name,
	None => "OneLauncher",
};

/// The version of the launcher (from `../Cargo.toml` env).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// =========== Default Ingress ===========
/// The amount of attempts to fetch a file before a request fails.
pub const FETCH_ATTEMPTS: usize = 3;
/// The total ingress capacity of the CLI, provided to [`indicatif`].
#[cfg(feature = "cli")]
pub const CLI_TOTAL_INGRESS: u64 = 1000;
/// The newline which we replace in metadata files.
pub const DUMMY_REPLACE_NEWLINE: &str = "\n";

// =========== Authentication ===========
/// Our Discord RPC client ID.
pub const DISCORD_RPC_CLIENT_ID: &str = "1274084193193955408";
/// Our Microsoft client ID.
pub const MICROSOFT_CLIENT_ID: &str = "9eac3a4e-8cdd-43ef-863e-49cd601b1f03";
/// Mojang/Microsoft client ID.
pub const MINECRAFT_CLIENT_ID: &str = "00000000402b5328";
/// Mojang/Microsoft login redirect URI.
pub const MINECRAFT_REDIRECT_URL: &str = "https://login.live.com/oauth20_desktop.srf";
/// Mojang/Microsoft login xboxlive scopes to get tokens.
pub const MINECRAFT_SCOPES: &str = "service::user.auth.xboxlive.com::MBI_SSL";

// =========== API ===========
// !!! URLS must NOT have a trailing slash. !!!
/// The Modrinth API base url.
pub const MODRINTH_API_URL: &str = "https://api.modrinth.com/v2";
/// The Modrinth V3 API base url. Used for things like fetching information about organizations.
pub const MODRINTH_V3_API_URL: &str = "https://api.modrinth.com/v3";
/// The Curseforge API base url.
pub const CURSEFORGE_API_URL: &str = "https://api.curseforge.com";
/// The Curseforge API key. Reserved for use in Polyfrost projects only. Do not use in other projects without permission from the Polyfrost team.
pub const CURSEFORGE_API_KEY: &str = "$2a$10$6utA1UNSmFPrE/Lh7b7ndeeGmiOkjKNY8kpFB0fsmE/d42ZAfFgCe";
/// The Minecraft game ID on Curseforge.
pub const CURSEFORGE_GAME_ID: u32 = 432;
/// Our metadata API base url.
pub const METADATA_API_URL: &str = "https://meta.polyfrost.org";
/// Featured packages API url.
pub const FEATURED_PACKAGES_URL: &str = "https://polyfrost.org/meta/onelauncher/featured.json";
/// <https://mclo.gs>/ API base url.
pub const MCLOGS_API_URL: &str = "https://api.mclo.gs/1";
/// <https://skyclient.co/> metadata base url.
pub const SKYCLIENT_BASE_URL: &str = "https://raw.githubusercontent.com/SkyblockClient/SkyblockClient-REPO/refs/heads/main/v1";

// =========== Paths ===========
/// The public `settings.json` file used to store the global [`Settings`] state.
///
/// [`Settings`]: crate::store::Settings
pub const SETTINGS_FILE: &str = "settings.json";
/// The public `authentication.json` file used to store the global [`MinecraftAuth`] state.
///
/// [`MinecraftAuth`]: crate::store::MinecraftAuth
pub const AUTH_FILE: &str = "authentication.json";
/// The public `processor.json` file used to store the global [`Processor`] state.
///
/// [`Processor`]: crate::store::Processor
pub const PROCESSOR_FILE: &str = "processor.json";

/// The current [`Settings`] format version, bumped for breaking changes.
/// If updated, a config file migration logic **NEEDS** to be implemented.
///
/// [`Settings`]: crate::store::Settings
pub const CURRENT_SETTINGS_FORMAT_VERSION: u32 = 1;

// =========== Hacky Mojang-spec OS constants ===========
#[cfg(target_os = "windows")]
pub const TARGET_OS: &str = "windows";

#[cfg(target_os = "macos")]
pub const TARGET_OS: &str = "osx";

#[cfg(target_os = "linux")]
pub const TARGET_OS: &str = "linux";

#[cfg(target_arch = "x86")]
pub const NATIVE_ARCH: &str = "32";

#[cfg(target_arch = "x86_64")]
pub const NATIVE_ARCH: &str = "64";

#[cfg(all(not(target_arch = "x86_64"), not(target_arch = "x86")))]
pub const NATIVE_ARCH: &str = "64";

// =========== Platform ===========
/// The default java executable on Windows or anything else.
pub const JAVA_BIN: &str = if cfg!(windows) { "javaw.exe" } else { "java" };

/// The bit width on x64 or x32 architectures.
pub const ARCH_WIDTH: &str = if cfg!(target_pointer_width = "64") {
	"64"
} else {
	"32"
};

/// Evaluates to `\r\n` on windows, and `\n` on everything else.
pub const LINE_ENDING: &str = if cfg!(windows) { "\r\n" } else { "\n" };
