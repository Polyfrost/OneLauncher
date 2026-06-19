// === API ===
pub const CURSEFORGE_API_KEY: &str = "$2a$10$6utA1UNSmFPrE/Lh7b7ndeeGmiOkjKNY8kpFB0fsmE/d42ZAfFgCe";
pub const DISCORD_CLIENT_ID: &str = "1426999264633946334";
pub const PLUS_BACKEND_URL: &str = match option_env!("ONECLIENT_PLUS_BACKEND_URL") {
	Some(url) => url,
	#[cfg(debug_assertions)]
	None => "https://plus-staging.polyfrost.org",
	#[cfg(not(debug_assertions))]
	None => "https://plus.polyfrost.org",
};

// === Meta ===
// Cannot contain trailing slash
pub const META_URL_BASE: &str =
	"https://raw.githubusercontent.com/Polyfrost/DataStorage/refs/heads/main/oneclient";
