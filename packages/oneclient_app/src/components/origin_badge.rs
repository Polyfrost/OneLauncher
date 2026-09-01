use freya::prelude::*;
use oneclient_core::clusters::Cluster;

use crate::theme::colors;
use crate::ui::border_all_color;

pub fn origin_badge(cluster: &Cluster) -> Element {
    let provisioned = cluster.is_provisioned();

    let text = if provisioned {
        "Default"
    } else {
        "Custom"
    };

    rect()
        .padding(Gaps::new_symmetric(3., 8.))
        .corner_radius(CornerRadius::new_all(999.))
        .background(colors::component_bg())
        .maybe(!provisioned, |el| {
            el.border(border_all_color(1., colors::component_border()))
        })
        .child(
            label()
                .text(text)
                .font_size(11.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .into_element()
}
