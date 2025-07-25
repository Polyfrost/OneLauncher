//! **`OneLauncher` Constants**
//!
//! Public constant variables and strings that are used throughout the launcher.

// =========== Default Ingress ===========
/// The total ingress capacity of the CLI, provided to [`indicatif`].
#[cfg(feature = "cli")]
pub const CLI_TOTAL_INGRESS: u64 = 1000;
/// The newline which we replace in metadata files.
pub const DUMMY_REPLACE_NEWLINE: &str = "\n";

// =========== Authentication ===========
/// Our Microsoft client ID.
pub const MICROSOFT_CLIENT_ID: &str = "9eac3a4e-8cdd-43ef-863e-49cd601b1f03";
/// Mojang/Microsoft login xboxlive scopes to get tokens.
pub const MINECRAFT_SCOPES: &str = "service::user.auth.xboxlive.com::MBI_SSL";

// =========== API ===========
// !!! URLS must NOT have a trailing slash. !!!
/// The Modrinth API base url.
pub const MODRINTH_API_URL: &str = "https://api.modrinth.com";
/// The Curseforge API base url.
pub const CURSEFORGE_API_URL: &str = "https://api.curseforge.com";
/// The Minecraft game ID on Curseforge.
pub const CURSEFORGE_GAME_ID: u32 = 432;
/// Our metadata API base url.
pub const METADATA_API_URL: &str = "https://meta.polyfrost.org";
/// Featured packages API url.
pub const FEATURED_PACKAGES_URL: &str = "https://polyfrost.org/meta/onelauncher/featured.json";
/// <https://mclo.gs>/ API base url.
pub const MCLOGS_API_URL: &str = "https://api.mclo.gs/1";
/// <https://skyclient.co/> metadata base url.
pub const SKYCLIENT_BASE_URL: &str =
	"https://raw.githubusercontent.com/SkyblockClient/SkyblockClient-REPO/refs/heads/main/v1";

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
pub const CLASSPATH_SEPARATOR: &str = if cfg!(windows) { ";" } else { ":" };
