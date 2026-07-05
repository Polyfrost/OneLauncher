use freya::animation::*;
use freya::prelude::*;
use freya::query::QueryStateData;
use freya::router::RouterContext;
use oneclient_core::clusters::Cluster;

use crate::components::{DynamicArt, Icon, IconType};
use crate::hooks::{use_active_cluster_id, use_clusters};
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::{border_all, border_all_color};
use crate::utils::sort_clusters_for_home;

const ROW_HEIGHT_PX: f32 = 208.0;
const CARD_GAP_PX: f32 = 24.0;
const MORE_TILE_WIDTH_PX: f32 = 96.0;
const MAX_CARD_WIDTH_PX: f32 = 480.0;
const CARD_MS: u64 = 460;
const STAGGER_MS: u64 = 85;
const CARD_RISE_PX: f32 = 48.0;

#[derive(PartialEq)]
pub struct RecentsRow;

impl Component for RecentsRow {
    fn render(&self) -> impl IntoElement {
        let clusters_query = use_clusters();
        let mut visible_slots = use_state(|| 1_usize);

        let clusters = {
            let reader = clusters_query.read();
            match &*reader.state() {
                QueryStateData::Settled { res: Ok(list), .. } => list.clone(),
                QueryStateData::Loading {
                    res: Some(Ok(list)),
                } => list.clone(),
                _ => Vec::new(),
            }
        };

        let sorted: Vec<Cluster> = sort_clusters_for_home(clusters);
        let slots = *visible_slots.read();
        let display = sorted.into_iter().take(slots).collect::<Vec<_>>();
        let count = display.len();

        let dep = (*use_active_cluster_id().read(), count);
        let intro = use_animation_with_dependencies(&dep, |conf, (_, count)| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            let total = CARD_MS + (count.saturating_sub(1) as u64) * STAGGER_MS;
            AnimNum::new(0., 1.).time(total.max(1)).function(Function::Linear)
        });
        let progress = intro.get().value();

        rect()
            .width(Size::fill())
            .height(Size::px(ROW_HEIGHT_PX))
            .content(Content::Flex)
            .on_sized(move |event: Event<SizedEventData>| {
                let width = event.data().area.width();
                let next = recent_card_slots_for_width(width).max(1);
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
                    count,
                    progress,
                }
                .into_element()
            }))
            .child(OtherVersionsTile)
    }
}

struct ClusterCard {
    cluster: Cluster,
    index: usize,
    count: usize,
    progress: f32,
}

impl PartialEq for ClusterCard {
    fn eq(&self, other: &Self) -> bool {
        self.cluster.id == other.cluster.id
            && self.index == other.index
            && self.count == other.count
            && self.progress == other.progress
    }
}

impl Component for ClusterCard {
    fn render(&self) -> impl IntoElement {
        let mut active_id = use_active_cluster_id();
        let active = *active_id.read() == Some(self.cluster.id);
        let mut hovering = use_state(|| false);

        let total = CARD_MS as f32 + (self.count.saturating_sub(1) as f32) * STAGGER_MS as f32;
        let elapsed = self.progress * total;
        let local =
            ((elapsed - self.index as f32 * STAGGER_MS as f32) / CARD_MS as f32).clamp(0., 1.);
        let eased = 1.0 - (1.0 - local).powi(3);
        let rise = (1.0 - eased) * CARD_RISE_PX;

        let title = format!(
            "{} {}",
            self.cluster.mc_version,
            self.cluster.mc_loader
        );

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
                        if active {
                            border_all_color(2., colors::brand())
                        } else if *hovering.read() {
                            border_all_color(1., colors::component_border_hover())
                        } else {
                            border_all(1.)
                        }
                        .alignment(BorderAlignment::Outer),
                    )
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
                    .on_press(on_press)
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
                                    .child(DynamicArt::for_cluster(&self.cluster).max_edge(512)),
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
struct OtherVersionsTile;

impl Component for OtherVersionsTile {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);

        rect()
            .width(Size::px(MORE_TILE_WIDTH_PX))
            .height(Size::fill())
            .corner_radius(CornerRadius::new_all(12.))
            .border(border_all(1.))
            .background(if *hovering.read() {
                colors::ghost_overlay_pressed()
            } else {
                colors::ghost_overlay_hover()
            })
            .blur(32.)
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
            .on_press(|_| {
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

fn recent_card_slots_for_width(row_width_px: f32) -> usize {
    const MORE_TILE_WIDTH: f32 = 96.0;
    const GAP: f32 = 24.0;
    const MIN_CARD_WIDTH: f32 = 300.0;

    if row_width_px <= MORE_TILE_WIDTH + GAP {
        return 0;
    }

    let available = row_width_px - MORE_TILE_WIDTH - GAP;
    let slot = MIN_CARD_WIDTH + GAP;
    (available / slot).floor() as usize
}
