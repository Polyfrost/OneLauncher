#[cfg(feature = "tauri")] mod proxy_tauri;
#[cfg(feature = "cli")] mod proxy_cli;
mod proxy_empty;

#[cfg(feature = "tauri")] pub use proxy_tauri::ProxyTauri;
#[cfg(feature = "cli")] pub use proxy_cli::ProxyCli;
pub use proxy_empty::ProxyEmpty;