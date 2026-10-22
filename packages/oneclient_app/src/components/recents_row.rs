use std::time::Duration;

use freya::animation::*;
use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::clusters::Cluster;

use freya::elements::image::{AspectRatio, ImageCover, ImageHandle, image};

use super::cluster_menu::cluster_menu_entries;
use super::dynamic_art::use_art_bytes;
use super::local_image::decode;
use crate::components::{ART_PREVIEW_EDGE, ContextMenu, DynamicArt, Icon, IconType, LocalImage};
use crate::hooks::{settled_or_loading, use_active_cluster_id, use_clusters};
use crate::motion::use_animations_enabled;
use crate::routes::Route;
use crate::theme;
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
const INTRO_ITEMS: usize = 6;
const MODPACK_ICON_PX: f32 = 72.0;
const MODPACK_ICON_EDGE: u32 = 192;
const MODPACK_BLUR: f32 = 4.0;
const MODPACK_TINT: Color = Color::from_argb(40, 21, 28, 34);
const MODPACK_BACKDROP: Color = Color::from_rgb(21, 28, 34);
const MODPACK_TITLE_INSET: f32 = 8.0;
const MODPACK_TITLE_LEFT: f32 = 10.5;
const MARQUEE_DELAY: Duration = Duration::from_millis(200);
const MARQUEE_HOLD: Duration = Duration::from_millis(700);
const MARQUEE_FRAME: Duration = Duration::from_millis(16);
const MARQUEE_SPEED_PX_PER_SEC: f32 = 60.0;
const MARQUEE_TRACK_PX: f32 = 4096.0;

#[derive(PartialEq)]
pub struct RecentsRow;

impl Component for RecentsRow {
    fn render(&self) -> impl IntoElement {
        let clusters_query = use_clusters();
        let platform = Platform::get();
        let root_size = platform.root_size;
        let scale_factor = platform.scale_factor;

        let clusters = settled_or_loading(&clusters_query).unwrap_or_default();

        let sorted: Vec<Cluster> = sort_clusters_for_home(clusters);

        let slots =
            recent_card_slots_for_window(root_size.read().width, *scale_factor.read() as f32);

        // Budget off cluster count not slot count an offset-only change makes the layout
        // engine reuse a card's cached area freezing survivors at pre-resize widths
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
            .horizontal()
            .spacing(CARD_GAP_PX)
            .children(display.iter().enumerate().map(|(index, cluster)| {
                ClusterCard {
                    cluster: cluster.clone(),
                    index,
                    items,
                    progress,
                    key: DiffKey::None,
                }
                .key(cluster.id)
                .into_element()
            }))
            .child(OtherVersionsTile {
                index: items.saturating_sub(1),
                items,
                progress,
            })
    }
}

fn intro_items(clusters: usize) -> usize {
    (clusters + 1).min(INTRO_ITEMS)
}

fn intro_ms(items: usize) -> u64 {
    CARD_MS + (items.saturating_sub(1) as u64) * STAGGER_MS
}

fn stagger_eased(progress: f32, index: usize, items: usize) -> f32 {
    let elapsed = progress * intro_ms(items) as f32;
    // Items past the budget share the last slot so every one still lands on a finished 1.0
    let start = index.min(items.saturating_sub(1)) as f32 * STAGGER_MS as f32;
    let local = ((elapsed - start) / CARD_MS as f32).clamp(0., 1.);
    1.0 - (1.0 - local).powi(3)
}


struct ClusterCard {
    cluster: Cluster,
    index: usize,
    items: usize,
    progress: f32,
    key: DiffKey,
}

impl KeyExt for ClusterCard {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl PartialEq for ClusterCard {
    fn eq(&self, other: &Self) -> bool {
        self.cluster.id == other.cluster.id
            && self.cluster.name == other.cluster.name
            && self.cluster.cover_path == other.cluster.cover_path
            && self.cluster.linked_modpack_hash == other.cluster.linked_modpack_hash
            && self.cluster.mc_loader == other.cluster.mc_loader
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
        let mut menu = use_state(|| None::<(f32, f32)>);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus().is_focused();

        let eased = stagger_eased(self.progress, self.index, self.items);
        let rise = (1.0 - eased) * CARD_RISE_PX;

        let title = if self.cluster.user_created {
            self.cluster.name.clone()
        } else {
            format!("{} {}", self.cluster.mc_version, self.cluster.mc_loader)
        };

        let cluster_id = self.cluster.id;
        let mod_tabs = !self.cluster.lacks_mod_loader();
        let modpack = self.cluster.linked_modpack_hash.is_some();
        let modpack_icon = self
            .cluster
            .modpack_icon_file()
            .filter(|icon| icon.is_file());
        let on_press = move |_| {
            *active_id.write() = Some(cluster_id);
        };

        let menu_overlay = (*menu.read()).map(|(x, y)| {
            let mut context = ContextMenu::new(x, y)
                .open_upwards()
                .title(title.clone())
                .on_close(move |_| menu.set(None));
            for (icon, label, route) in
                cluster_menu_entries(cluster_id)
                    .into_iter()
                    .filter(|(_, _, route)| {
                        mod_tabs
                            || !matches!(
                                route,
                                Route::ClusterMods { .. } | Route::ClusterShaders { .. }
                            )
                    })
            {
                context = context.action(icon, label, move |()| {
                    let _ = RouterContext::get().push(route.clone());
                });
            }
            context.into_element()
        });

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
                    .on_press(on_press)
                    .on_secondary_down(move |e: Event<PressEventData>| {
                        if let PressEventData::Mouse(m) = e.data() {
                            menu.set(Some((
                                m.global_location.x as f32,
                                m.global_location.y as f32,
                            )));
                        }
                    })
                    .child(
                        rect()
                            .width(Size::fill())
                            .height(Size::fill())
                            .child(if modpack {
                                BlurredArt {
                                    art: DynamicArt::for_cluster(&self.cluster)
                                        .max_edge(ART_PREVIEW_EDGE),
                                }
                                .into_element()
                            } else {
                                rect()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .position(Position::new_absolute())
                                    .layer(Layer::Relative(1))
                                    .child(
                                        DynamicArt::for_cluster(&self.cluster)
                                            .max_edge(ART_PREVIEW_EDGE),
                                    )
                                    .into_element()
                            })
                            .maybe_child(modpack_icon.map(|icon| {
                                rect()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .position(Position::new_absolute())
                                    .center()
                                    .layer(Layer::Relative(5))
                                    .child(modpack_badge(icon))
                                    .into_element()
                            }))
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .position(Position::new_absolute())
                                    .padding(if modpack {
                                        Gaps::new(12., MODPACK_TITLE_INSET, 12., MODPACK_TITLE_LEFT)
                                    } else {
                                        Gaps::new_symmetric(12., 24.)
                                    })
                                    .main_align(Alignment::End)
                                    .cross_align(Alignment::Start)
                                    .layer(Layer::Relative(4))
                                    .background(
                                        LinearGradient::new()
                                            .angle(0.0)
                                            .stop((Color::from_argb(0, 25, 25, 25), 24.519))
                                            .stop((Color::from_af32rgb(0.75, 17, 17, 21), 65.)),
                                    )
                                    .child(if modpack {
                                        MarqueeTitle {
                                            text: title,
                                            hovered: *hovering.read(),
                                        }
                                        .into_element()
                                    } else {
                                        title_label(title).into_element()
                                    }),
                            )
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::fill())
                                    .position(Position::new_absolute())
                                    .layer(Layer::Relative(7))
                                    .background(if *hovering.read() {
                                        Color::from_af32rgb(0.2, 0, 0, 0)
                                    } else {
                                        Color::TRANSPARENT
                                    }),
                            ),
                    ),
            )
            .maybe_child(menu_overlay)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

fn title_label(text: String) -> Label {
    label()
        .text(text)
        .font_size(32.)
        .font_weight(FontWeight::SEMI_BOLD)
        .max_lines(1)
        .color(colors::fg_primary())
}

#[derive(PartialEq)]
struct MarqueeTitle {
    text: String,
    hovered: bool,
}

impl Component for MarqueeTitle {
    fn render(&self) -> impl IntoElement {
        let animations = use_animations_enabled();
        let mut box_width = use_state(|| 0f32);
        let mut text_width = use_state(|| 0f32);
        let offset = use_state(|| 0f32);
        let run = use_state(|| 0u64);

        let overflow = (*text_width.read() - *box_width.read()).max(0.);
        let deps = (self.hovered && animations, overflow);
        use_side_effect_with_deps(&deps, move |&(hovered, overflow)| {
            let mut run = run;
            let mut offset = offset;
            let current = *run.peek() + 1;
            run.set(current);

            if !hovered || overflow < 1. {
                offset.set(0.);
                return;
            }

            spawn(async move {
                let alive = move || *run.peek() == current;
                tokio::time::sleep(MARQUEE_DELAY).await;
                if !alive() {
                    return;
                }
                if !slide(offset, 0., -overflow, &alive).await {
                    return;
                }
                tokio::time::sleep(MARQUEE_HOLD).await;
                if !alive() {
                    return;
                }
                slide(offset, -overflow, 0., &alive).await;
            });
        });

        rect()
            .width(Size::fill())
            .overflow(Overflow::Clip)
            .on_sized(move |event: Event<SizedEventData>| {
                let width = event.data().area.width();
                if (width - *box_width.peek()).abs() > 0.5 {
                    box_width.set(width);
                }
            })
            .child(
                rect().width(Size::px(MARQUEE_TRACK_PX)).child(
                    rect()
                        .offset_x(*offset.read())
                        .on_sized(move |event: Event<SizedEventData>| {
                            let width = event.data().area.width();
                            if (width - *text_width.peek()).abs() > 0.5 {
                                text_width.set(width);
                            }
                        })
                        .child(title_label(self.text.clone())),
                ),
            )
    }
}

async fn slide(mut offset: State<f32>, from: f32, to: f32, alive: &impl Fn() -> bool) -> bool {
    let distance = (to - from).abs();
    let total = Duration::from_secs_f32(distance / MARQUEE_SPEED_PX_PER_SEC);
    let started = std::time::Instant::now();

    loop {
        if !alive() {
            return false;
        }
        let progress = (started.elapsed().as_secs_f32() / total.as_secs_f32().max(0.001)).min(1.);
        offset.set(from + (to - from) * progress);
        if progress >= 1. {
            return true;
        }
        tokio::time::sleep(MARQUEE_FRAME).await;
    }
}

#[derive(PartialEq)]
struct BlurredArt {
    art: DynamicArt,
}

impl Component for BlurredArt {
    fn render(&self) -> impl IntoElement {
        let (key, bytes) = use_art_bytes(&self.art);
        let mut cache = use_state(|| None::<(String, ImageHandle)>);

        let cached = cache
            .read()
            .clone()
            .filter(|(cached_key, _)| *cached_key == key)
            .map(|(_, holder)| holder);
        let holder = cached.or_else(|| {
            let holder = decode(&bytes)?;
            cache.set(Some((key.clone(), holder.clone())));
            Some(holder)
        });

        let backdrop = rect()
            .width(Size::fill())
            .height(Size::fill())
            .position(Position::new_absolute())
            .overflow(Overflow::Clip)
            .background(MODPACK_BACKDROP);

        let Some(holder) = holder else {
            return backdrop.into_element();
        };

        backdrop
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .overflow(Overflow::Clip)
                    .layer(Layer::Relative(1))
                    .child(
                        image(holder)
                            .width(Size::fill())
                            .height(Size::fill())
                            .aspect_ratio(AspectRatio::Max)
                            .image_cover(ImageCover::Center),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::fill())
                    .position(Position::new_absolute())
                    .overflow(Overflow::Clip)
                    .layer(Layer::Relative(3))
                    .backdrop_blur(MODPACK_BLUR)
                    .background(MODPACK_TINT),
            )
            .into_element()
    }
}

fn modpack_badge(icon: std::path::PathBuf) -> Element {
    rect()
        .width(Size::px(MODPACK_ICON_PX))
        .height(Size::px(MODPACK_ICON_PX))
        .min_width(Size::px(MODPACK_ICON_PX))
        .corner_radius(CornerRadius::new_all(14.))
        .overflow(Overflow::Clip)
        .child(LocalImage::new(icon, MODPACK_ICON_EDGE, false))
        .into_element()
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
            .backdrop_blur(32.)
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

fn recent_card_slots_for_window(window_width_px: f32, scale_factor: f32) -> usize {
    let scale = if scale_factor > 0. && scale_factor.is_finite() {
        scale_factor
    } else {
        1.
    };
    recent_card_slots_for_width(window_width_px / scale - theme::HOME_PADDING_PX * 2.)
}

/// `n` cards occupy `n` gaps (n-1 between cards one before the tile) so `n * (MIN + GAP) + MORE`
/// must fit
/// Always returns at least 1 the card carries no `min_width` and just renders narrower
fn recent_card_slots_for_width(row_width_px: f32) -> usize {
    if !row_width_px.is_finite() {
        return 1;
    }

    let available = row_width_px - MORE_TILE_WIDTH_PX;
    let slot = MIN_CARD_WIDTH_PX + CARD_GAP_PX;
    (available / slot).floor().max(1.0) as usize
}
