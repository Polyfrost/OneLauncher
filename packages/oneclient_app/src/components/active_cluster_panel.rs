use freya::animation::*;
use freya::prelude::*;
use freya::router::RouterContext;

use crate::components::{Button, Icon, IconType, LobbyHeroChip};
use oneclient_common::parse_mc_version;
use oneclient_core::clusters::ClusterKind;

use crate::hooks::{
    settled_or_loading, use_active_cluster_id, use_clusters, use_dispatch, use_game_snapshot,
    use_launcher, use_version_metadata,
};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::{colors, text_size};
use crate::utils::{builtin_oneclient_title, sort_clusters_for_home};
use crate::view::app::{
    launch_button_label, launch_button_state, launch_syncing, shared_dir_taken,
};

#[derive(PartialEq)]
pub struct ActiveClusterPanel;

impl Component for ActiveClusterPanel {
    fn render(&self) -> impl IntoElement {
        let clusters_query = use_clusters();
        let mut active_id = use_active_cluster_id();
        let dispatch = use_dispatch();
        let game = use_game_snapshot();
        let launcher = use_launcher();

        let clusters = settled_or_loading(&clusters_query).unwrap_or_default();

        let sorted = sort_clusters_for_home(clusters);

        if active_id.read().is_none()
            && let Some(first) = sorted.first()
        {
            *active_id.write() = Some(first.id);
        }

        let active = active_id
            .read()
            .and_then(|id| sorted.iter().find(|c| c.id == id).cloned())
            .or_else(|| sorted.first().cloned());

        let dep = active.as_ref().map(|c| c.id);
        let intro = use_animation_with_dependencies(&dep, |conf, _| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            AnimNum::new(0., 1.)
                .time(440)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let p = intro.get().value();
        let slide_x = (p - 1.0) * 48.0;

        let parsed = active
            .as_ref()
            .and_then(|c| parse_mc_version(&c.mc_version));
        let metadata = use_version_metadata(
            parsed.as_ref().map(|p| p.major),
            parsed.and_then(|p| p.key()),
            active.as_ref().map(|c| c.mc_loader),
        );

        let theme = colors::active_theme();
        let subtitle_ink = if theme.colors.light || theme.liquid_glass {
            Color::from_rgb(241, 242, 247)
        } else {
            colors::on_art_secondary()
        };
        let Some(cluster) = active else {
            return rect()
                .vertical()
                .width(Size::fill())
                .main_align(Alignment::Center)
                .child(
                    label()
                        .text(t("oneclient.components.active_cluster_panel.empty"))
                        .font_size(text_size(24.))
                        .color(subtitle_ink),
                )
                .child(LobbyHeroChip);
        };

        let version = format!("{} {}", cluster.mc_version, cluster.mc_loader);
        let (title, subtitle) = if let Some(title) = builtin_oneclient_title(&cluster) {
            let mut details = vec![cluster.mc_loader.to_string()];
            details.extend(metadata.map(|m| m.name.clone()));
            (title, details.join(" \u{b7} "))
        } else if cluster.user_created && cluster.kind != ClusterKind::OneClient {
            (cluster.name.clone(), version)
        } else {
            let mut parts = vec!["OneClient".to_string()];
            let title = if cluster.user_created {
                parts.push(version);
                cluster.name.clone()
            } else {
                version
            };
            parts.extend(metadata.map(|m| m.name.clone()));
            (title, parts.join(" \u{b7} "))
        };
        let cluster_id = cluster.id;
        let syncing = launch_syncing(&launcher, cluster.uses_bundles());
        let shared_dir_taken = shared_dir_taken(&game, &sorted, cluster_id);

        rect()
            .vertical()
            .width(Size::fill())
            .main_align(Alignment::Center)
            .cross_align(Alignment::Start)
            .offset_x(slide_x)
            .opacity(p)
            .child(home_heading(
                title,
                subtitle,
                subtitle_ink,
                theme.pixel_icons && theme.colors.light,
            ))
            .child(
                rect()
                    .horizontal()
                    .spacing(8.)
                    .cross_align(Alignment::Center)
                    .margin(Gaps::new(8., 0., 0., 0.))
                    .child(launch_button(
                        cluster_id,
                        dispatch,
                        launch_button_state(&game, cluster_id, syncing, shared_dir_taken),
                    ))
                    .child(cluster_settings_button(cluster_id)),
            )
            .child(LobbyHeroChip)
    }
}

pub(super) fn home_heading(
    title: String,
    subtitle: String,
    subtitle_ink: Color,
    space_subtitle: bool,
) -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .child(
            label()
                .text(title)
                .width(Size::fill())
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(text_size(56.))
                .line_height(1.1)
                .font_weight(FontWeight::BOLD)
                .color(if colors::is_light() {
                    Color::WHITE
                } else {
                    colors::on_art()
                }),
        )
        .child(
            label()
                .text(subtitle)
                .maybe(space_subtitle, |el| {
                    el.letter_spacing(0.5).margin(Gaps::new(3., 0., 0., 0.))
                })
                .font_size(text_size(16.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(subtitle_ink),
        )
}

fn launch_button(
    cluster_id: i64,
    dispatch: crate::Actions,
    state: (&'static str, bool),
) -> impl IntoElement {
    let (label, enabled) = state;
    let button = Button::new()
        .primary()
        .medium()
        .font_size(text_size(16.))
        .font_weight(FontWeight::from(450))
        .enabled(enabled)
        .padding(Gaps::new_symmetric(8., 24.))
        .on_press(move |_| {
            if enabled {
                dispatch.launch_cluster(cluster_id);
            }
        });
    launch_button_label(button, label)
}

fn cluster_settings_button(cluster_id: i64) -> impl IntoElement {
    Button::new()
        .ghost()
        .icon()
        .foreground(colors::on_art())
        .tooltip(t("oneclient.components.active_cluster_panel.settings"))
        .on_press(move |_| {
            let _ = RouterContext::get().push(Route::ClusterOverview { cluster_id });
        })
        .child(
            Icon::new(IconType::Settings04)
                .size(20.)
                .color(colors::on_art()),
        )
}
