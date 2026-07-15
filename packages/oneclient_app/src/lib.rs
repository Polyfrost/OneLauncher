// TODO: Remove this once freya has some workaround for this
#![allow(float_literal_f32_fallback)]
// Deeply nested async call chains through instrumented oneclient_core fns exceed the default limit
#![recursion_limit = "256"]

mod assets;
mod bridge;
mod components;
pub mod hooks;
mod layout;
mod notifications;
pub mod platform;
mod routes;
pub mod theme;
mod ui;
pub mod updater;
pub(crate) mod utils;
mod view;

pub mod constants;

pub use assets::AppAssets;
pub use bridge::*;
pub use components::ConfirmLinkOverlay;
pub use hooks::*;
pub use routes::{Route, router};
