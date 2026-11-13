mod console;
mod create;
mod list;
mod mods;
mod motd;
mod overview;
mod pages;
mod settings;

use std::time::Instant;

use freya::prelude::*;
use oneclient_common::parse_mc_version;
use oneclient_core::servers::LocalServer;

pub use console::{ServerConsole, ServerLiveConsole};
pub use list::Servers;
pub use overview::ServerOverview;
pub use mods::ServerMods;
pub use pages::{ServerBackups, ServerPlayers, ServerWorlds};
pub use settings::ServerSettings;

use crate::components::{ART_PREVIEW_EDGE, DynamicArt};
use crate::hooks::ServerIcon;
use crate::i18n::t;
use crate::servers::ServerStatus;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

pub fn version_art(server: &LocalServer) -> DynamicArt {
    match parse_mc_version(&server.mc_version) {
        Some(parsed) => DynamicArt::for_version(parsed.major, parsed.key(), Some(server.loader)),
        None => DynamicArt::fallback(),
    }
}

pub fn server_icon(server: &LocalServer, icon: Option<ServerIcon>, size: f32) -> Element {
    rect()
        .width(Size::px(size))
        .height(Size::px(size))
        .corner_radius(rounded(size / 6.))
        .overflow(Overflow::Clip)
        .child(match icon {
            Some(icon) => ImageViewer::new((icon.key, icon.bytes))
                .width(Size::fill())
                .height(Size::fill())
                .aspect_ratio(AspectRatio::Max)
                .image_cover(ImageCover::Center)
                .into_element(),
            None => version_art(server)
                .max_edge(ART_PREVIEW_EDGE)
                .into_element(),
        })
        .into_element()
}

/// Saves the art the launcher shows in place of a missing icon as server-icon.png,
/// so Minecraft's server list shows the same picture
#[derive(PartialEq)]
pub struct IconSeeder {
    pub server: LocalServer,
}

impl Component for IconSeeder {
    fn render(&self) -> impl IntoElement {
        let (key, bytes) = version_art(&self.server).max_edge(ART_PREVIEW_EDGE).use_bytes();
        // The bundled background stands in while the version art is still loading
        let loaded = (!bytes.is_empty() && !key.ends_with(crate::layout::HOME_BACKGROUND_ASSET))
            .then_some((key, bytes));
        let id = self.server.id.clone();
        use_side_effect_with_deps(&loaded, move |loaded| {
            let Some((_, bytes)) = loaded.clone() else {
                return;
            };
            let id = id.clone();
            crate::servers::run(t("oneclient.servers.settings.motd.icon_failed"), async move {
                oneclient_core::servers::set_icon_bytes(&oneclient_core::servers::server_dir(&id)?, bytes.to_vec()).await
            });
        });
        rect()
    }
}

pub fn loader_label(server: &LocalServer) -> String {
    format!("{} {}", server.loader, server.mc_version)
}

pub fn status_label(status: ServerStatus) -> &'static str {
    match status {
        ServerStatus::Stopped => t("oneclient.servers.status.stopped"),
        ServerStatus::Installing => t("oneclient.servers.status.installing"),
        ServerStatus::Starting => t("oneclient.servers.status.starting"),
        ServerStatus::Running => t("oneclient.servers.status.running"),
        ServerStatus::Stopping => t("oneclient.servers.status.stopping"),
    }
}

pub fn status_line(status: ServerStatus, server: &LocalServer) -> String {
    format!("{} \u{b7} {}", status_label(status), loader_label(server))
}

pub fn fmt_uptime(started: Instant) -> String {
    let secs = started.elapsed().as_secs();
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

pub fn fmt_bytes(bytes: u64) -> String {
    const GB: f64 = 1024. * 1024. * 1024.;
    const MB: f64 = 1024. * 1024.;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.0} MB", b / MB)
    } else {
        format!("{:.0} KB", b / 1024.)
    }
}

pub const DASH: &str = "\u{2014}";

pub fn card() -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .padding(Gaps::new_all(22.))
        .spacing(14.)
        .corner_radius(rounded(14.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
}

pub fn card_title(title: impl Into<String>, subtitle: impl Into<String>) -> Element {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .spacing(3.)
        .child(
            label()
                .text(title.into())
                .font_size(text_size(18.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(subtitle.into())
                .font_size(text_size(12.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

pub fn card_head(
    title: impl Into<String>,
    subtitle: impl Into<String>,
    trailing: Option<Element>,
) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(card_title(title, subtitle))
        .maybe_child(trailing)
        .into_element()
}

/// The entry text color vanilla draws the MOTD and player count in (0x808080)
const MC_GRAY: Color = Color::from_rgb(128, 128, 128);
/// One GUI pixel, the preview is drawn as the game looks at GUI scale 2
const GUI: f32 = 2.;
/// Vanilla's fixed server list row: 305 wide, a 32 square icon, text 3 past it
const ROW_W: f32 = 305.;
const ICON: f32 = 32.;
const TEXT_GAP: f32 = 3.;
const LINE_H: f32 = 9.;
/// The bundled font draws one GUI pixel per 100 of its 1000 units, so 10 px per GUI px
const FONT: f32 = 10.;

/// shortcut: a five-bar stand-in for the ping sprite, the real one is a texture
fn ping_bars() -> Element {
    rect()
        .horizontal()
        .width(Size::px(10. * GUI))
        .height(Size::px(8. * GUI))
        .main_align(Alignment::End)
        .cross_align(Alignment::End)
        .spacing(GUI)
        .children((0..5).map(|i| {
            rect()
                .width(Size::px(GUI))
                .height(Size::px((i as f32 + 1.) * 1.5 * GUI))
                .background(Color::from_rgb(0, 255, 33))
                .into_element()
        }))
        .into_element()
}

/// How the server looks in Minecraft's Multiplayer list while it is online, at the
/// game's fixed row size: the name, the player count and ping, then the MOTD in up to
/// two lines with its formatting codes applied
pub fn list_preview(
    server: &LocalServer,
    icon: Option<ServerIcon>,
    motd: &str,
    players: String,
) -> Element {
    let text_w = ROW_W - ICON - TEXT_GAP;
    let line = |text: String, color: Color| {
        label()
            .text(text)
            .max_lines(1)
            .font_size(FONT * GUI)
            .line_height(LINE_H / FONT)
            .color(color)
    };

    let row = rect()
        .horizontal()
        .width(Size::px(ROW_W * GUI))
        .height(Size::px(ICON * GUI))
        .spacing(TEXT_GAP * GUI)
        .font_family(crate::theme::MINECRAFT_FONT)
        .child(server_icon(server, icon, ICON * GUI))
        .child(
            rect()
                .vertical()
                .width(Size::px(text_w * GUI))
                .height(Size::fill())
                .padding(Gaps::new(GUI, 0., 0., 0.))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .height(Size::px(11. * GUI))
                        .content(Content::Flex)
                        .spacing(2. * GUI)
                        .padding(Gaps::new(0., 5. * GUI, 0., 0.))
                        .child(line(server.name.clone(), Color::WHITE).width(Size::flex(1.0)))
                        .child(line(players, MC_GRAY))
                        .child(ping_bars()),
                )
                .child(
                    rect()
                        .width(Size::fill())
                        .font_size(FONT * GUI)
                        .child(motd::motd_text(motd, MC_GRAY, LINE_H / FONT)),
                ),
        );

    rect()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(18., 0.))
        .center()
        .corner_radius(rounded(10.))
        .background(Color::from_rgb(10, 12, 14))
        .overflow(Overflow::Clip)
        .child(
            // The outline the game draws around the selected entry
            rect()
                .padding(Gaps::new_all(GUI))
                .background(Color::BLACK)
                .border(border_all_color(GUI / 2., Color::from_rgb(128, 128, 128)))
                .child(row),
        )
        .into_element()
}

