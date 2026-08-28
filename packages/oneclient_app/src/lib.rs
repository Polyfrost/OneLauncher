// TODO Remove this once freya has some workaround for this
#![allow(float_literal_f32_fallback)]
// Instrumented oneclient_core async call chains exceed the default limit
#![recursion_limit = "256"]

mod assets;
pub mod chat;
mod components;
pub mod hooks;
pub mod events;
mod install;
mod launcher;
pub mod state;
mod transfer;
mod layout;
mod motion;
mod notifications;
pub mod platform;
pub mod recovery;
mod routes;
pub mod theme;
mod ui;
pub mod updater;
pub(crate) mod utils;
mod view;

pub mod constants;

pub use assets::AppAssets;
pub use events::EventPump;
pub use state::{AppChannel, AppState};
pub use components::ConfirmLinkOverlay;
pub use hooks::*;
pub use routes::{Route, router};
