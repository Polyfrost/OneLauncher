#[cfg(feature = "tauri")] pub mod proxy_tauri;
#[cfg(feature = "cli")] pub mod proxy_cli;
pub mod proxy_empty;

#[cfg(feature = "tauri")] pub use proxy_tauri::ProxyTauri;
#[cfg(feature = "cli")] pub use proxy_cli::ProxyCli;
pub use proxy_empty::ProxyEmpty;