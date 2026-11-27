use freya::prelude::*;
use oneclient_core::clusters::Cluster;

use crate::components::cluster_menu::{
    InstanceOverviewButton, InstanceOverviewSurface, use_instance_menu_trigger,
};
use crate::components::{ART_PREVIEW_EDGE, ClusterContextMenu, ClusterMenuTarget, DynamicArt};
use crate::i18n::{t, tf};
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::utils::{GridSelection, ReleaseLine, line_art_key, line_title};

/// Height used when the caller does not size the card from its column width
const DEFAULT_HEIGHT_PX: f32 = 150.;
const COVER_RADIUS_PX: f32 = 14.;
/// Narrow covers lean towards 3:2 so the art keeps some height, wide ones go 16:9
const NARROW_COVER_WIDTH_PX: f32 = 200.;
const WIDE_COVER_WIDTH_PX: f32 = 300.;
const NARROW_COVER_ASPECT: f32 = 3. / 2.;
const WIDE_COVER_ASPECT: f32 = 16. / 9.;

pub struct VersionCard {
    pub key: GridSelection,
    pub art: DynamicArt,
    pub title: String,
    pub caption: String,
    pub count: usize,
    play_id: i64,
    show_play: bool,
    uses_bundles: bool,
    pub selected: bool,
    pub height: f32,
    pub on_press: EventHandler<Event<PressEventData>>,
    pub menu: Option<ClusterMenuTarget>,
}

impl VersionCard {
    pub fn menu(mut self, target: Option<ClusterMenuTarget>) -> Self {
        self.menu = target;
        self
    }

    pub fn selection_only(mut self) -> Self {
        self.show_play = false;
        self
    }

    /// Total height of the artwork tile
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// Cover width / height for a card of the given width
    pub fn cover_aspect(card_width: f32) -> f32 {
        let t = ((card_width - NARROW_COVER_WIDTH_PX)
            / (WIDE_COVER_WIDTH_PX - NARROW_COVER_WIDTH_PX))
            .clamp(0., 1.);
        NARROW_COVER_ASPECT + (WIDE_COVER_ASPECT - NARROW_COVER_ASPECT) * t
    }

    /// Height of the full-bleed illustration at this width
    pub fn height_for_width(card_width: f32) -> f32 {
        let width = card_width.max(1.);
        (width / Self::cover_aspect(width)).round()
    }

    pub(super) fn instance_caption(cluster: &Cluster) -> String {
        if !cluster.is_isolated() {
            "OneClient".into()
        } else if cluster.linked_modpack_hash.is_some() {
            t("oneclient.clusters.page.type.modpack").into()
        } else {
            cluster.mc_loader.to_string()
        }
    }

    pub fn new(
        line: ReleaseLine,
        clusters: &[Cluster],
        target: &Cluster,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            key: GridSelection::Line(line),
            art: DynamicArt::for_version(line.major, line_art_key(line, clusters), None)
                .max_edge(ART_PREVIEW_EDGE),
            title: line_title(line, clusters),
            caption: "OneClient".into(),
            count: clusters.len(),
            play_id: target.id,
            show_play: true,
            uses_bundles: target.uses_bundles(),
            selected,
            height: DEFAULT_HEIGHT_PX,
            on_press: on_press.into(),
            menu: None,
        }
    }

    pub fn for_instance(
        cluster: &Cluster,
        selected: bool,
        on_press: impl Into<EventHandler<Event<PressEventData>>>,
    ) -> Self {
        Self {
            key: GridSelection::Instance(cluster.id),
            art: DynamicArt::for_cluster(cluster).max_edge(ART_PREVIEW_EDGE),
            title: if cluster.is_isolated() {
                cluster.name.clone()
            } else {
                cluster.mc_version.clone()
            },
            caption: Self::instance_caption(cluster),
            count: 1,
            play_id: cluster.id,
            show_play: true,
            uses_bundles: cluster.uses_bundles(),
            selected,
            height: DEFAULT_HEIGHT_PX,
            on_press: on_press.into(),
            menu: None,
        }
    }
}

impl PartialEq for VersionCard {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.art == other.art
            && self.title == other.title
            && self.caption == other.caption
            && self.count == other.count
            && self.play_id == other.play_id
            && self.show_play == other.show_play
            && self.uses_bundles == other.uses_bundles
            && self.selected == other.selected
            && self.height == other.height
            && self.menu == other.menu
    }
}

impl Component for VersionCard {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let menu_at = use_state(|| None);
        let mut card_width = use_state(|| 0.);

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);

        let selected = self.selected;
        let hovered = *hovering.read();
        let keyboard = focus() == Focus::Keyboard;
        let compact_caption = *card_width.read() < 210.;
        let caption = if compact_caption && self.caption == "OneClient" && self.show_play {
            ""
        } else {
            &self.caption
        };

        // Selection is a brand ring around the whole card. Keyboard focus is a neutral ring
        // outside it, so a focused selection shows both instead of one hiding the other.
        let rim = if selected {
            colors::brand()
        } else if hovered {
            colors::component_border_hover()
        } else {
            colors::page()
        };
        let outer = if keyboard {
            colors::fg_strong()
        } else {
            colors::page()
        };
        use_instance_menu_trigger(menu_at, self.menu.is_some(), self.on_press.clone())
            .key(self.key)
            .vertical()
            .width(Size::fill())
            .height(Size::px(self.height))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .maybe(!self.show_play, |el| {
                el.a11y_alt(format!("{} \u{b7} {}", self.title, self.caption))
                    .cursor(CursorIcon::Pointer)
            })
            .on_pointer_over(move |_| {
                hovering.set_if_modified(true);
            })
            .on_pointer_move(move |_| hovering.set_if_modified(true))
            .on_pointer_leave(move |_| {
                hovering.set_if_modified(false);
            })
            .maybe_child(self.menu.clone().map(|target| {
                ClusterContextMenu {
                    target,
                    position: menu_at,
                    upwards: false,
                }
                .into_element()
            }))
            .child(
                rect()
                    .expanded()
                    .corner_radius(rounded(COVER_RADIUS_PX))
                    .background(outer)
                    .padding(1.)
                    .child(
                        rect()
                            .expanded()
                            .corner_radius(rounded(COVER_RADIUS_PX - 1.))
                            .background(rim)
                            .padding(2.)
                            .child(
                                rect()
                                    .expanded()
                                    .corner_radius(rounded(COVER_RADIUS_PX - 3.))
                                    .on_sized(move |e: Event<SizedEventData>| {
                                        card_width.set_if_modified(e.area.width())
                                    })
                                    .overflow(Overflow::Clip)
                                    .child(super::art_zoom::ArtZoom {
                                        highlighted: hovered || keyboard,
                                        art: self.art.clone().into_element(),
                                    })
                                    .child(artwork_caption(&self.title, caption, self.count))
                                    .maybe(self.menu.is_some(), |el| {
                                        el.child(
                                            rect()
                                                .position(
                                                    Position::new_absolute().right(8.).top(7.),
                                                )
                                                .layer(Layer::Relative(7))
                                                .child(InstanceOverviewButton {
                                                    position: menu_at,
                                                    surface: InstanceOverviewSurface::Artwork {
                                                        reveal: hovered || keyboard,
                                                    },
                                                    name: self.title.clone(),
                                                }),
                                        )
                                    })
                                    .maybe_child(self.show_play.then(|| {
                                        rect()
                                            .position(Position::new_absolute().right(8.).bottom(7.))
                                            .layer(Layer::Relative(7))
                                            .child(super::InstancePlay {
                                                cluster_id: self.play_id,
                                                uses_bundles: self.uses_bundles,
                                                name: self.title.clone(),
                                                list: false,
                                            })
                                    })),
                            ),
                    ),
            )
    }
}

pub(super) fn artwork_caption(title: &str, detail: &str, count: usize) -> Rect {
    let theme = colors::active_theme();
    let (radius, opacity, line) = if theme.liquid_glass {
        (13., 213, 1.25)
    } else {
        (6., 238, 1.)
    };
    let mut caption = paragraph()
        .max_width(Size::fill())
        .max_lines(1)
        .text_overflow(TextOverflow::Ellipsis)
        .font_size(text_size(14.))
        .span(
            Span::new(title.to_owned())
                .font_weight(FontWeight::SEMI_BOLD)
                .color(if theme.colors.light {
                    colors::fg_primary()
                } else {
                    Color::WHITE
                }),
        );
    let count = (count > 1).then(|| tf("oneclient.components.version_card.instances", &[&count]));
    for detail in std::iter::once(detail)
        .chain(count.as_deref())
        .filter(|detail| !detail.is_empty())
    {
        caption = caption.span(
            Span::new(format!(" \u{b7} {detail}"))
                .font_size(text_size(12.))
                .color(if theme.colors.light {
                    colors::fg_secondary()
                } else {
                    Color::from_rgb(229, 232, 238)
                }),
        );
    }

    rect()
        .position(Position::new_absolute().left(0.).bottom(0.))
        .width(Size::fill())
        .padding(Gaps::new(0., 48., 10., 9.))
        .cross_align(Alignment::Start)
        .interactive(false)
        .layer(Layer::Relative(6))
        .child(
            rect()
                .max_width(Size::fill())
                .height(Size::px(26.))
                .main_align(Alignment::Center)
                .padding(Gaps::new_symmetric(0., 10.))
                .corner_radius(rounded(radius))
                .background(if theme.colors.light {
                    colors::component_bg().with_a(opacity)
                } else {
                    Color::from_argb(opacity, 22, 24, 28)
                })
                .border(
                    border_all_color(
                        line,
                        Color::from_argb(if theme.colors.light { 105 } else { 238 }, 9, 11, 14),
                    )
                    .alignment(BorderAlignment::Inner),
                )
                .child(caption),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_is_only_used_as_the_custom_instance_caption() {
        use oneclient_db::models::ClusterKind;
        let mut cluster = crate::view::onboarding::test_support::cluster(1);
        assert_eq!(VersionCard::instance_caption(&cluster), "OneClient");
        cluster.kind = ClusterKind::Modded;
        assert_eq!(
            VersionCard::instance_caption(&cluster),
            cluster.mc_loader.to_string()
        );
        cluster.linked_modpack_hash = Some("test-pack".into());
        assert_eq!(
            VersionCard::instance_caption(&cluster),
            t("oneclient.clusters.page.type.modpack")
        );
    }

    #[test]
    fn cover_goes_from_three_two_to_sixteen_nine() {
        assert!((VersionCard::cover_aspect(120.) - NARROW_COVER_ASPECT).abs() < 1e-4);
        assert!((VersionCard::cover_aspect(200.) - NARROW_COVER_ASPECT).abs() < 1e-4);
        assert!((VersionCard::cover_aspect(300.) - WIDE_COVER_ASPECT).abs() < 1e-4);
        assert!((VersionCard::cover_aspect(520.) - WIDE_COVER_ASPECT).abs() < 1e-4);
        let mid = VersionCard::cover_aspect(250.);
        assert!(mid > NARROW_COVER_ASPECT && mid < WIDE_COVER_ASPECT);
    }

    #[test]
    fn tile_height_tracks_the_cover_width() {
        let cover = VersionCard::height_for_width;
        assert!((cover(200.) - 133.).abs() < 1e-4);
        assert!((cover(320.) - 180.).abs() < 1e-4);
        assert!(cover(0.) >= 0.);
        let mut last = 0.;
        for width in (120u16..=520).step_by(10) {
            let height = VersionCard::height_for_width(f32::from(width));
            assert!(height >= last, "height shrank at {width}px");
            last = height;
        }
    }
}
