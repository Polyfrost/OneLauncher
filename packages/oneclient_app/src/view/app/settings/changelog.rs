use freya::animation::{
    AnimNum, Ease, Function, OnChange, OnCreation, use_animation_with_dependencies,
};
use freya::prelude::*;

use super::settings_page;
use crate::components::{Icon, IconType, Markdown, MarkdownStyle};
use crate::hooks::{
    changelog_entries, changelog_error, changelog_is_loading, latest_changelog_version,
    use_changelog, use_dispatch, use_settings_snapshot,
};
use crate::theme::colors;

#[derive(PartialEq)]
pub struct SettingsChangelog;

impl Component for SettingsChangelog {
    fn render(&self) -> impl IntoElement {
        let query = use_changelog();
        let dispatch = use_dispatch();
        let settings = use_settings_snapshot().settings;
        let mut marked = use_state(|| None::<String>);
        let installed_version = env!("CARGO_PKG_VERSION").to_string();

        // Opening this page counts as reading it `marked` avoids re-sending while the settings snapshot catches up
        if let Some(latest) = latest_changelog_version(&query) {
            let already_marked = marked.peek().as_deref() == Some(latest.as_str());
            let already_seen = settings.seen_changelog_version.as_deref() == Some(latest.as_str());

            if !already_marked && !already_seen {
                marked.set(Some(latest.clone()));
                dispatch.set_seen_changelog_version(latest);
            }
        }

        if changelog_is_loading(&query) {
            return settings_page()
                .child(
                    label()
                        .text("Loading changelog...")
                        .font_size(14.)
                        .color(colors::fg_secondary()),
                )
                .into_element();
        }

        if let Some(error) = changelog_error(&query) {
            return settings_page()
                .child(
                    label()
                        .text(format!("Couldn't load changelog: {error}"))
                        .font_size(14.)
                        .color(colors::fg_secondary()),
                )
                .into_element();
        }

        let entries = changelog_entries(&query).unwrap_or_default();

        settings_page()
            .children(entries.into_iter().enumerate().map(|(i, entry)| {
                let current = entry.version == installed_version;
                ReleaseCard {
                    version: entry.version,
                    current,
                    body: entry.body,
                    initially_open: i == 0,
                }
                .into_element()
            }))
            .into_element()
    }
}

/// `ChevronDown` swung a quarter turn anticlockwise points right
const CHEVRON_CLOSED_DEG: f32 = -90.;

#[derive(PartialEq)]
struct ReleaseCard {
    version: String,
    current: bool,
    body: String,
    initially_open: bool,
}

impl Component for ReleaseCard {
    fn render(&self) -> impl IntoElement {
        let mut open = use_state(|| self.initially_open);
        let is_open = *open.read();

        // One chevron that swings so the arrow tracks the section instead of cutting to a different glyph
        let swing = use_animation_with_dependencies(&is_open, |conf, open| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            let (from, to) = if *open {
                (CHEVRON_CLOSED_DEG, 0.)
            } else {
                (0., CHEVRON_CLOSED_DEG)
            };
            AnimNum::new(from, to)
                .time(180)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let chevron_deg = swing.get().value();

        let title = if self.current {
            format!("{} (Currently Installed)", self.version)
        } else {
            self.version.clone()
        };

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(8.)
            .padding(Gaps::new_symmetric(12., 16.))
            .corner_radius(CornerRadius::new_all(12.))
            .background(colors::page_elevated())
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .main_align(Alignment::SpaceBetween)
                    .cursor(CursorIcon::Pointer)
                    .on_press(move |_| {
                        let next = !*open.peek();
                        *open.write() = next;
                    })
                    .child(
                        label()
                            .text(title)
                            .font_size(18.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        Icon::new(IconType::ChevronDown)
                            .size(18.)
                            .rotate(chevron_deg),
                    ),
            )
            .maybe_child(is_open.then(|| {
                if self.body.trim().is_empty() {
                    label()
                        .text("No changes recorded for this version.")
                        .font_size(12.)
                        .color(colors::fg_secondary())
                        .into_element()
                } else {
                    Markdown::new(self.body.clone())
                        .width(Size::fill())
                        .style(MarkdownStyle {
                            color: colors::fg_primary(),
                            color_link: colors::code_info(),
                            color_code: colors::fg_primary(),
                            background_code: colors::component_bg(),
                            background_blockquote: colors::component_bg(),
                            border_blockquote: colors::brand(),
                            background_divider: colors::component_border(),
                            headings: [18., 16., 14., 13., 12., 12.],
                            paragraph_size: 12.,
                            code_font_size: 11.,
                            ..MarkdownStyle::default()
                        })
                        .into_element()
                }
            }))
            .into_element()
    }
}
