#![warn(unused_import_braces, missing_debug_implementations)]
#![deny(unused_must_use)]

pub mod auth;
pub mod constants;
pub mod error;
pub mod game;
pub mod logger;
pub mod utils;
pub mod settings;

pub use error::*;
pub use logger::start_logger;

pub struct AppState {
    pub settings: settings::SettingsManager,
    pub clients: game::client_manager::ClientManager,
}

unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

impl AppState {
    pub fn new() -> crate::Result<Self> {
        Ok(Self {
            settings: settings::SettingsManager::new()?,
            clients: game::client_manager::ClientManager::new()?,
        })
    }
}
