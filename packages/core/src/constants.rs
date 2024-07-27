//! **OneLauncher Constants**
//!
//! Public constant variables and strings that are used throughout the launcher.
//! Can be shared publically, any encrypted secrets should be stored in [`crate::store::credentials`].

// =========== Core Metadata ===========
/// The name of the launcher.
pub const NAME: &str = "OneLauncher";
/// The version of the launcher
pub const VERSION: &str = "1.0.0-alpha.1"; // todo: env

// =========== Authentication ===========
/// The Discord RPC client ID.
pub const DISCORD_RPC_CLIENT_ID: &str = "1234567890000000";

/// Our Microsoft client ID.
pub const MICROSOFT_CLIENT_ID: &str = "9eac3a4e-8cdd-43ef-863e-49cd601b1f03";
/// Mojang/Microsoft client ID.
pub const MINECRAFT_CLIENT_ID: &str = "00000000402b5328";
/// Mojang/Microsoft login redirect URI.
pub const MINECRAFT_REDIRECT_URL: &str = "https://login.live.com/oauth20_desktop.srf";
/// Mojang/Microsoft login xboxlive scopes to get tokens.
pub const MINECRAFT_SCOPES: &str = "service::user.auth.xboxlive.com::MBI_SSL";

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

// TODO: Add more architectures
