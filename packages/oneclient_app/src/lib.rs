// TODO: Remove this once freya has some workaround for this
#![allow(float_literal_f32_fallback)]

mod assets;
mod bridge;
mod components;
pub mod hooks;
mod layout;
mod notifications;
pub mod platform;
mod routes;
pub mod theme;
pub mod updater;
mod ui;
pub(crate) mod utils;
mod view;

pub mod constants;

pub use assets::AppAssets;
pub use bridge::*;
pub use components::ConfirmLinkOverlay;
pub use hooks::*;
pub use routes::{Route, router};
