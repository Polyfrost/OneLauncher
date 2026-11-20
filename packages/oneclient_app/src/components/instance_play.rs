use freya::prelude::*;
use freya::radio::use_radio;

use super::tooltip::use_tooltip_anchor;
use crate::components::{Icon, IconType, TooltipPlacement};
use crate::hooks::{use_clusters, use_dispatch, use_launcher};
use crate::i18n::{t, tf};
use crate::state::AppChannel;
use crate::theme::{self, colors, rounded};
use crate::ui::border_all_color;
use crate::view::app::{launch_button_state, launch_syncing, shared_dir_taken};

/// Edge of the square the button occupies, the caller reserves this much room
pub(crate) const INSTANCE_PLAY_PX: f32 = 32.;

const ICON_PX: f32 = 18.;
/// Lands on a whole number of device pixels per cell at the common scale factors
const PIXEL_ICON_PX: f32 = 24.;
/// The outline triangle sits a little left of its view box, so it is nudged back to centre
const ICON_NUDGE_PX: f32 = 1.5;
/// Same ink as the artwork caption pill so both read as one family over art
const OUTLINE: Color = Color::from_argb(238, 9, 11, 14);

/// Compact Play action for one instance, for cards and rows that already act as a button
/// themselves. It stops its press from reaching the surface underneath.
#[derive(Clone, PartialEq)]
pub(crate) struct InstancePlay {
    pub cluster_id: i64,
    pub uses_bundles: bool,
    pub name: String,
    /// List rows: a quiet brand tint instead of the solid fill the artwork cards use
    pub list: bool,
}

fn direct_play(
    cluster_id: i64,
    ready: impl Fn() -> bool,
    launch: impl Fn(i64),
) -> impl Fn(Event<PressEventData>) {
    move |event| {
        event.stop_propagation();
        if ready() {
            launch(cluster_id);
        }
    }
}

/// Spoken name and tooltip. A blocked button says why instead of only going dim.
pub(crate) fn instance_play_label(name: &str, state: &str, enabled: bool) -> String {
    if enabled {
        tf("oneclient.components.instance_play.launch", &[&name])
    } else {
        tf(
            "oneclient.components.instance_play.unavailable",
            &[&name, &state],
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Glass,
    Pixel,
    Accent,
}

impl Component for InstancePlay {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let mut pressing = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let keyboard = focus() == Focus::Keyboard;

        let dispatch = use_dispatch();
        let launcher = use_launcher();
        let radio = use_radio(AppChannel::Game);
        let clusters_query = use_clusters();

        let cluster_id = self.cluster_id;
        let uses_bundles = self.uses_bundles;
        let syncing = launch_syncing(&launcher, uses_bundles);

        // Both stores are read in place: a clone of the library per card adds up on a full grid
        let (state_label, enabled) = {
            let app = radio.read();
            let reader = clusters_query.read();
            let query = reader.state();
            let dir_taken = match query.ok() {
                Some(clusters) => shared_dir_taken(&app.game, clusters, cluster_id),
                None => false,
            };
            let (label, enabled) = launch_button_state(&app.game, cluster_id, syncing, dir_taken);
            if syncing && label == t("oneclient.app.launch.launch") {
                (t("oneclient.app.launch.syncing"), enabled)
            } else {
                (label, enabled)
            }
        };

        let text = instance_play_label(&self.name, state_label, enabled);
        let tooltip = use_tooltip_anchor(Some(text.clone().into()), TooltipPlacement::Top);

        let theme = colors::active_theme();
        let look = if theme.liquid_glass {
            Look::Glass
        } else if theme.pixel_icons {
            Look::Pixel
        } else {
            Look::Accent
        };
        let hovered = enabled && *hovering.read();
        let pressed = enabled && *pressing.read();

        let radius = match look {
            Look::Glass => CornerRadius::new_all(INSTANCE_PLAY_PX / 2.),
            Look::Pixel => CornerRadius::new_all(0.),
            Look::Accent => rounded(8.),
        };
        let accent_fill = if !enabled {
            colors::brand_disabled()
        } else if pressed {
            colors::brand_pressed()
        } else if hovered {
            colors::brand_hover()
        } else {
            colors::brand()
        };
        let ink = match look {
            _ if self.list || look == Look::Glass => {
                colors::fg_primary().with_a(if enabled { 255 } else { 150 })
            }
            _ if enabled => colors::on_brand(),
            _ => colors::on_brand_disabled(),
        };

        let face = rect()
            .width(Size::px(INSTANCE_PLAY_PX))
            .height(Size::px(INSTANCE_PLAY_PX))
            .min_width(Size::px(INSTANCE_PLAY_PX))
            .min_height(Size::px(INSTANCE_PLAY_PX));
        let face = match look {
            _ if self.list => face.corner_radius(radius).background(if pressed {
                colors::component_bg_pressed()
            } else if hovered {
                colors::component_bg_hover()
            } else {
                colors::page_elevated()
            }),
            Look::Glass => {
                let tint = if theme.colors.light {
                    Color::WHITE
                } else {
                    Color::from_rgb(14, 17, 22)
                };
                let alpha = match (pressed, hovered) {
                    (true, _) => 196,
                    (_, true) => 176,
                    _ => 148,
                };
                theme::glass::panel(face, &theme, INSTANCE_PLAY_PX / 2., tint.with_a(alpha))
            }
            Look::Pixel => face.corner_radius(radius).background(accent_fill),
            Look::Accent => face.corner_radius(radius).background(accent_fill).shadow(
                Shadow::new()
                    .blur(8.)
                    .spread(0.)
                    .x(0.)
                    .y(2.)
                    .color(Color::from_af32rgb(
                        if colors::is_light() { 0.22 } else { 0.42 },
                        0,
                        0,
                        0,
                    )),
            ),
        };

        // The theme's own edge lives on an inner overlay so the keyboard ring can take the
        // outer border: light ring outside, dark edge inside, readable on any backdrop
        let edge = match (look, keyboard) {
            (_, true) => Some(border_all_color(2., OUTLINE)),
            (_, false) if self.list => Some(border_all_color(
                if look == Look::Pixel { 2. } else { 1. },
                if hovered || pressed {
                    colors::component_border_hover()
                } else {
                    colors::component_border()
                },
            )),
            (Look::Glass, false) => Some(border_all_color(
                1.25,
                if theme.colors.light {
                    OUTLINE.with_a(96)
                } else {
                    OUTLINE
                },
            )),
            (Look::Pixel, false) => Some(border_all_color(2., OUTLINE)),
            (Look::Accent, false) => None,
        };

        let icon_px = if look == Look::Pixel {
            PIXEL_ICON_PX
        } else {
            ICON_PX
        };
        let nudge = if look == Look::Pixel {
            0.
        } else {
            ICON_NUDGE_PX
        };

        let button = face
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .a11y_alt(text)
            .maybe(keyboard, |el| {
                el.border(border_all_color(2., Color::WHITE).alignment(BorderAlignment::Outer))
            })
            .cursor(if enabled {
                CursorIcon::Pointer
            } else {
                CursorIcon::NotAllowed
            })
            .on_pointer_down(move |_| {
                pressing.set_if_modified(true);
            })
            .on_global_pointer_up(move |_| {
                pressing.set_if_modified(false);
            })
            .on_pointer_over(move |_| {
                hovering.set_if_modified(true);
            })
            .on_pointer_out(move |_| {
                hovering.set_if_modified(false);
                pressing.set_if_modified(false);
            })
            // Covers click, tap and Enter/Space on the focused button. Always swallowed, so a
            // blocked Play never falls through and selects or opens the card behind it
            .on_press(direct_play(
                cluster_id,
                move || {
                    let app = radio.read();
                    let syncing = launch_syncing(&app.launcher, uses_bundles);
                    let reader = clusters_query.read();
                    let query = reader.state();
                    let taken = query
                        .ok()
                        .is_some_and(|list| shared_dir_taken(&app.game, list, cluster_id));
                    launch_button_state(&app.game, cluster_id, syncing, taken).1
                },
                move |id| dispatch.launch_cluster(id),
            ))
            .maybe_child(edge.map(|edge| {
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute().top(0.).left(0.))
                    .interactive(false)
                    .corner_radius(radius)
                    .border(edge.alignment(BorderAlignment::Inner))
            }))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .center()
                    .padding(Gaps::new(0., 0., 0., nudge))
                    .child(Icon::new(IconType::Play).size(icon_px).color(ink)),
            );

        tooltip.attach(button)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freya_testing::TestingRunner;
    use std::{cell::Cell, rc::Rc};

    #[derive(Default)]
    struct Probe {
        ready: Cell<bool>,
        selected: Cell<bool>,
        launched: Cell<Option<i64>>,
    }

    fn card() -> impl IntoElement {
        let probe = use_consume::<Rc<Probe>>();
        let guard = probe.clone();
        let launch = probe.clone();
        rect()
            .width(Size::px(180.))
            .height(Size::px(80.))
            .on_press(move |_| probe.selected.set(true))
            .child(
                rect()
                    .width(Size::px(32.))
                    .height(Size::px(32.))
                    .on_press(direct_play(
                        42,
                        move || guard.ready.get(),
                        move |id| launch.launched.set(Some(id)),
                    )),
            )
    }

    #[test]
    fn play_targets_its_instance_and_never_selects_the_parent_card() {
        let (mut runner, probe) = TestingRunner::new(
            card,
            Size2D::new(200., 100.),
            |runner| runner.provide_root_context(|| Rc::new(Probe::default())),
            1.,
        );
        runner.sync_and_update();
        runner.click_cursor((15., 15.));
        runner.sync_and_update();
        assert_eq!(probe.launched.get(), None);
        assert!(!probe.selected.get());
        probe.ready.set(true);
        runner.click_cursor((15., 15.));
        runner.sync_and_update();
        assert_eq!(probe.launched.get(), Some(42));
        assert!(!probe.selected.get());
        runner.click_cursor((95., 40.));
        runner.sync_and_update();
        assert!(probe.selected.get());
    }
}
