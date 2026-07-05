use freya::animation::*;
use freya::prelude::*;
use freya::query::QueryStateData;
use freya::router::RouterContext;

use crate::components::{Button, Icon, IconType};
use crate::hooks::{use_active_cluster_id, use_clusters, use_dispatch, use_game_snapshot};
use crate::routes::Route;
use crate::theme::colors;
use crate::utils::sort_clusters_for_home;
use crate::view::app::launch_button_state;

#[derive(PartialEq)]
pub struct ActiveClusterPanel;

impl Component for ActiveClusterPanel {
    fn render(&self) -> impl IntoElement {
        let clusters_query = use_clusters();
        let mut active_id = use_active_cluster_id();
        let dispatch = use_dispatch();
        let game = use_game_snapshot();

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

        let Some(cluster) = active else {
            return rect()
                .vertical()
                .width(Size::fill())
                .main_align(Alignment::Center)
                .child(
                    label()
                        .text("No versions yet")
                        .font_size(24.)
                        .color(colors::fg_secondary()),
                );
        };

        let title = format!("{} {}", cluster.mc_version, cluster.mc_loader);
        let subtitle = cluster.name.clone();
        let cluster_id = cluster.id;

        rect()
            .vertical()
            .width(Size::fill())
            .main_align(Alignment::Center)
            .cross_align(Alignment::Start)
            .offset_x(slide_x)
            .opacity(p)
            .child(
                label()
                    .text(title)
                    .font_size(56.)
                    .line_height(1.1)
                    .font_weight(FontWeight::BOLD)
                    .color(colors::fg_primary()),
            )
            .child(
                label()
                    .text(subtitle)
                    .font_size(16.)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .horizontal()
                    .spacing(8.)
                    .cross_align(Alignment::Center)
                    .margin(Gaps::new(8., 0., 0., 0.))
                    .child(launch_button(cluster_id, dispatch, launch_button_state(&game, cluster_id)))
                    .child(cluster_settings_button(cluster_id)),
            )
    }
}

fn launch_button(
    cluster_id: i64,
    dispatch: crate::BridgeDispatch,
    state: (&'static str, bool),
) -> impl IntoElement {
    let (label, enabled) = state;
    Button::new()
        .primary()
        .medium()
        .font_size(16.)
        .font_weight(FontWeight::from(450))
        .enabled(enabled)
        .padding(Gaps::new_symmetric(8., 24.))
        .on_press(move |_| {
            if enabled {
                dispatch.launch_cluster(cluster_id);
            }
        })
        .text(label)
}

fn cluster_settings_button(cluster_id: i64) -> impl IntoElement {
    Button::new()
        .ghost()
        .icon()
        .on_press(move |_| {
            let _ = RouterContext::get().push(Route::ClusterOverview { cluster_id });
        })
        .child(Icon::new(IconType::Settings04).size(20.))
}
