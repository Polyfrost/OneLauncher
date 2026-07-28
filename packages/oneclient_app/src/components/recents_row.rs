use freya::animation::*;
use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::clusters::Cluster;

use crate::components::{ART_PREVIEW_EDGE, DynamicArt, Icon, IconType};
use crate::hooks::{settled_or_loading, use_active_cluster_id, use_clusters};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::{border_all, border_all_color};
use crate::utils::sort_clusters_for_home;

const ROW_HEIGHT_PX: f32 = 208.0;
const CARD_GAP_PX: f32 = 24.0;
const MORE_TILE_WIDTH_PX: f32 = 96.0;
const MIN_CARD_WIDTH_PX: f32 = 300.0;
const MAX_CARD_WIDTH_PX: f32 = 480.0;
const CARD_MS: u64 = 460;
const STAGGER_MS: u64 = 85;
const CARD_RISE_PX: f32 = 48.0;
/// Items the intro staggers individually; past this they rise together.
const INTRO_ITEMS: usize = 6;

#[derive(PartialEq)]
pub struct RecentsRow;

impl Component for RecentsRow {
    fn render(&self) -> impl IntoElement {
        let clusters_query = use_clusters();
        let mut visible_slots = use_state(|| 1_usize);

        let clusters = settled_or_loading(&clusters_query).unwrap_or_default();

        let sorted: Vec<Cluster> = sort_clusters_for_home(clusters);
        let slots = *visible_slots.read();

        // Budgeted off the clusters we have, never off the slots we can show. An
        // offset is the one thing a card must not change on a frame where the row
        // is re-measured: the layout engine treats an offset-only change as "inner
        // layout" and hands back the card's *cached* area instead of measuring it,
        // so a rerun triggered by the slot count would freeze the surviving cards
        // at their pre-resize widths while the new ones come in at the new width.
        let items = intro_items(sorted.len());

        let display = sorted.into_iter().take(slots).collect::<Vec<_>>();

        let intro = use_animation_with_dependencies(&items, |conf, items| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            AnimNum::new(0., 1.)
                .time(intro_ms(*items).max(1))
                .function(Function::Linear)
        });
        let progress = intro.get().value();

        rect()
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT_PX))
            .content(Content::Flex)
            .on_sized(move |event: Event<SizedEventData>| {
                let next = recent_card_slots_for_width(event.data().area.width());
                if next != *visible_slots.peek() {
                    *visible_slots.write() = next;
                }
            })
            .horizontal()
            .spacing(CARD_GAP_PX)
            .children(display.iter().enumerate().map(|(index, cluster)| {
                ClusterCard {
                    cluster: cluster.clone(),
                    index,
                    items,
                    progress,
                }
                .into_element()
            }))
            .child(OtherVersionsTile {
                index: items.saturating_sub(1),
                items,
                progress,
            })
    }
}

/// Items to stagger for `clusters` recents, counting the "other versions" tile.
fn intro_items(clusters: usize) -> usize {
    (clusters + 1).min(INTRO_ITEMS)
}

fn intro_ms(items: usize) -> u64 {
    CARD_MS + (items.saturating_sub(1) as u64) * STAGGER_MS
}

fn stagger_eased(progress: f32, index: usize, items: usize) -> f32 {
    let elapsed = progress * intro_ms(items) as f32;
    // Anything past the budget shares the last slot, so every item still lands on
    // a finished 1.0 rather than freezing part-way with a leftover offset.
    let start = index.min(items.saturating_sub(1)) as f32 * STAGGER_MS as f32;
    let local = ((elapsed - start) / CARD_MS as f32).clamp(0., 1.);
    1.0 - (1.0 - local).powi(3)
}

struct ClusterCard {
    cluster: Cluster,
    index: usize,
    items: usize,
    progress: f32,
}

impl PartialEq for ClusterCard {
    fn eq(&self, other: &Self) -> bool {
        self.cluster.id == other.cluster.id
            && self.index == other.index
            && self.items == other.items
            && self.progress == other.progress
    }
}

impl Component for ClusterCard {
    fn render(&self) -> impl IntoElement {
        let mut active_id = use_active_cluster_id();
        let active = *active_id.read() == Some(self.cluster.id);
        let mut hovering = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let eased = stagger_eased(self.progress, self.index, self.items);
        let rise = (1.0 - eased) * CARD_RISE_PX;

        let title = format!("{} {}", self.cluster.mc_version, self.cluster.mc_loader);

        let cluster_id = self.cluster.id;
        let on_press = move |_| {
            *active_id.write() = Some(cluster_id);
        };

        rect()
            .key(self.cluster.id)
            .width(Size::flex(1.0))
            .max_width(Size::px(MAX_CARD_WIDTH_PX))
            .height(Size::fill())
            .offset_y(rise)
            .opacity(eased)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .corner_radius(CornerRadius::new_all(12.))
                    .overflow(Overflow::Clip)
                    .border(
                        if active || focused {
                            border_all_color(2., colors::brand())
                        } else if *hovering.read() {
                            border_all_color(1., colors::component_border_hover())
                        } else {
                            border_all(1.)
                        }
                        .alignment(BorderAlignment::Outer),
                    )
                    .a11y_id(a11y_id)
                    .a11y_focusable(true)
                    .a11y_role(AccessibilityRole::Button)
                    .on_pointer_over(move |_| {
                        if !*hovering.peek() {
                            *hovering.write() = true;
                        }
                    })
                    .on_pointer_out(move |_| {
                        *hovering.write() = false;
                    })
                    .shadow(
                        Shadow::new()
                            .blur(24.)
                            .spread(12.)
                            .x(0.)
                            .y(0.)
                            .color(Color::from_af32rgb(0.3, 0, 0, 0)),
                    )
                    .on_all_press(on_press)
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .position(Position::new_absolute())
                                    .layer(Layer::Relative(1))
                                    .child(DynamicArt::for_cluster(&self.cluster).max_edge(ART_PREVIEW_EDGE)),
                            )
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .position(Position::new_absolute())
                                    .padding(Gaps::new_symmetric(12., 24.))
                                    .main_align(Alignment::End)
                                    .cross_align(Alignment::Start)
                                    .layer(Layer::Relative(3))
                                    .background(
                                        LinearGradient::new()
                                            .angle(0.0)
                                            .stop((Color::from_argb(0, 25, 25, 25), 24.519))
                                            .stop((Color::from_af32rgb(0.75, 17, 17, 21), 65.)),
                                    )
                                    .child(
                                        label()
                                            .text(title)
                                            .font_size(32.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .position(Position::new_absolute())
                                    .layer(Layer::Relative(5))
                                    .background(if *hovering.read() {
                                        Color::from_af32rgb(0.2, 0, 0, 0)
                                    } else {
                                        Color::TRANSPARENT
                                    }),
                            ),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct OtherVersionsTile {
    index: usize,
    items: usize,
    progress: f32,
}

impl Component for OtherVersionsTile {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let eased = stagger_eased(self.progress, self.index, self.items);
        let rise = (1.0 - eased) * CARD_RISE_PX;

        rect()
            .width(Size::px(MORE_TILE_WIDTH_PX))
            .height(Size::fill())
            .offset_y(rise)
            .opacity(eased)
            .corner_radius(CornerRadius::new_all(12.))
            .border(
                if focused {
                    border_all_color(2., colors::brand())
                } else {
                    border_all(1.)
                }
                .alignment(BorderAlignment::Outer),
            )
            .background(if *hovering.read() {
                colors::ghost_overlay_pressed()
            } else {
                colors::ghost_overlay_hover()
            })
            .blur(32.)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| {
                *hovering.write() = true;
            })
            .on_pointer_leave(move |_| {
                *hovering.write() = false;
            })
            .shadow(
                Shadow::new()
                    .blur(24.)
                    .spread(12.)
                    .x(0.)
                    .y(0.)
                    .color(Color::from_af32rgb(0.3, 0, 0, 0)),
            )
            .on_all_press(|_| {
                let _ = RouterContext::get().push(Route::Clusters {});
            })
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .center()
                    .child(Icon::new(IconType::DotsGrid).size(50.)),
            )
    }
}

/// How many cards fit beside the "other versions" tile at `row_width_px`.
///
/// `n` cards sit in `n` gaps' worth of spacing — `n - 1` between the cards and
/// one before the tile — so `n * (MIN + GAP) + MORE` has to fit. One card is
/// always shown: below that width it just renders narrower than the minimum,
/// which is why the card carries no `min_width`. With one, a stale count during
/// a live resize used to push the row past its bounds and leave a card cut off
/// or stretched until the next measurement landed.
fn recent_card_slots_for_width(row_width_px: f32) -> usize {
    if !row_width_px.is_finite() {
        return 1;
    }

    let available = row_width_px - MORE_TILE_WIDTH_PX;
    let slot = MIN_CARD_WIDTH_PX + CARD_GAP_PX;
    (available / slot).floor().max(1.0) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_fits(slots: usize, row_width_px: f32) -> bool {
        let cards = slots as f32 * MIN_CARD_WIDTH_PX;
        let gaps = slots as f32 * CARD_GAP_PX;
        cards + gaps + MORE_TILE_WIDTH_PX <= row_width_px
    }

    #[test]
    fn slots_fill_the_row_without_overflowing_it() {
        for width in (200..3200).step_by(4) {
            let width = width as f32;
            let slots = recent_card_slots_for_width(width);

            assert!(slots >= 1, "{width}px left no room for a card");
            if slots > 1 {
                assert!(row_fits(slots, width), "{slots} cards overflow {width}px");
            }
            assert!(
                !row_fits(slots + 1, width),
                "{width}px had room for {} cards",
                slots + 1
            );
        }
    }

    #[test]
    fn every_item_settles_when_the_intro_ends() {
        // A card left with a leftover offset keeps changing it, and an offset-only
        // change is what makes the layout engine reuse a card's cached width.
        for clusters in 0..24 {
            let items = intro_items(clusters);
            for index in 0..=clusters {
                assert_eq!(
                    stagger_eased(1.0, index, items),
                    1.0,
                    "item {index} of {clusters} never finished rising"
                );
            }
        }
    }

    #[test]
    fn the_intro_runs_the_same_length_whatever_fits_on_screen() {
        // The window width decides how many cards show, so anything the intro is
        // measured against has to be independent of it.
        assert_eq!(intro_items(3), intro_items(3));
        assert_eq!(intro_ms(intro_items(30)), intro_ms(INTRO_ITEMS));
        assert_eq!(intro_ms(intro_items(0)), CARD_MS);
    }
}
