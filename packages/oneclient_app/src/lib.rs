// TODO Remove this once freya has some workaround for this
#![allow(float_literal_f32_fallback)]
// Instrumented oneclient_core async call chains exceed the default limit
#![recursion_limit = "256"]

mod assets;
pub mod cli;
mod components;
pub mod essential;
pub mod events;
pub(crate) mod file_content;
pub mod hooks;
mod install;
pub mod ipc;
mod launcher;
mod layout;
pub mod microsoft_java;
mod motion;
mod notifications;
pub mod platform;
pub mod protocol;
pub mod recovery;
mod routes;
pub mod shortcut;
pub mod state;
pub mod theme;
mod transfer;
mod ui;
pub mod updater;
pub(crate) mod utils;
mod view;

pub mod constants;

pub use assets::AppAssets;
pub use components::{ConfirmLinkOverlay, EssentialConfirmOverlay};
pub use events::EventPump;
pub use hooks::*;
pub use routes::{Route, router};
pub use state::{AppChannel, AppState};
