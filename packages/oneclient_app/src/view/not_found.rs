use freya::prelude::*;

use crate::theme;

#[derive(PartialEq, Clone)]
pub struct NotFound {
    pub path: Vec<String>,
}

impl Component for NotFound {
    fn render(&self) -> impl IntoElement {
        tracing::warn!("Route not found: /{}", self.path.join("/"));

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(40.)
            .spacing(16.)
            .child(
                label()
                    .text("404 - Not Found")
                    .font_size(32.)
                    .font_weight(FontWeight::BOLD)
                    .color(theme::colors::fg_primary()),
            )
            .child(
                label()
                    .text("The page you are looking for does not exist.")
                    .font_size(16.)
                    .color(theme::colors::fg_secondary()),
            )
    }
}
