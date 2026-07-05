use freya::prelude::*;
use oneclient_core::packages::domain::GameLoader;

use crate::components::DynamicArt;
use crate::theme::colors;
use crate::ui::border_all_color;

#[derive(PartialEq, Clone)]
pub struct ClusterLandscapeArt {
    art: DynamicArt,
    selected: bool,
}

impl ClusterLandscapeArt {
    pub fn for_version(
        major: u32,
        minor: Option<u32>,
        loader: Option<GameLoader>,
        selected: bool,
    ) -> Self {
        Self {
            art: DynamicArt::for_version(major, minor, loader).max_edge(512),
            selected,
        }
    }

    pub fn for_major(major: u32, selected: bool) -> Self {
        Self {
            art: DynamicArt::for_major(major).max_edge(512),
            selected,
        }
    }
}

impl Component for ClusterLandscapeArt {
    fn render(&self) -> impl IntoElement {
        let border = if self.selected {
            border_all_color(2., colors::brand())
        } else {
            border_all_color(2., colors::component_border())
        };

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .corner_radius(CornerRadius::new_all(12.))
            .overflow(Overflow::Clip)
            .border(border.alignment(BorderAlignment::Inner))
            .child(self.art.clone())
    }
}
