use freya::prelude::*;

use crate::components::{Button, Icon, IconType};
use crate::theme::colors;
use crate::ui::border_all_color;

fn page_window(current: usize, total: usize, window: usize) -> (usize, usize) {
    if total <= window {
        return (0, total);
    }
    let end = (current + window / 2 + 1).min(total);
    let start = end.saturating_sub(window);
    (start, end)
}

#[derive(PartialEq)]
pub struct Pagination {
    page: State<usize>,
    total_pages: usize,
    window: usize,
}

impl Pagination {
    pub fn new(page: State<usize>, total_pages: usize) -> Self {
        Self {
            page,
            total_pages,
            window: 5,
        }
    }

    #[allow(dead_code)]
    pub fn window(mut self, window: usize) -> Self {
        self.window = window.max(1);
        self
    }
}

impl Component for Pagination {
    fn render(&self) -> impl IntoElement {
        let page = self.page;
        let total_pages = self.total_pages;
        let current = *page.read();
        let last = total_pages.saturating_sub(1);

        let icon_btn = move |target: usize, enabled: bool, icon: IconType| {
            let mut page = page;
            Button::new()
                .secondary()
                .icon()
                .enabled(enabled)
                .on_press(move |_| page.set(target))
                .child(Icon::new(icon).size(14.).color(if enabled {
                    colors::fg_primary()
                } else {
                    colors::fg_secondary().with_a(90)
                }))
                .into_element()
        };

        let num_btn = move |target: usize| {
            let mut page = page;
            let is_current = target == current;

            if is_current {
                return rect()
                    .center()
                    .width(Size::px(32.))
                    .height(Size::px(32.))
                    .corner_radius(CornerRadius::new_all(8.))
                    .background(colors::component_bg_pressed())
                    .border(border_all_color(1., colors::brand()))
                    .child(
                        label()
                            .text(format!("{}", target + 1))
                            .font_size(12.)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .color(colors::fg_primary()),
                    )
                    .into_element();
            }

            Button::new()
                .secondary()
                .width(Size::px(32.))
                .height(Size::px(32.))
                .padding(Gaps::new_symmetric(6., 0.))
                .on_press(move |_| page.set(target))
                .child(
                    label()
                        .text(format!("{}", target + 1))
                        .font_size(12.)
                        .font_weight(FontWeight::NORMAL)
                        .max_lines(1)
                        .color(colors::fg_secondary()),
                )
                .into_element()
        };

        let ellipsis = || {
            rect()
                .center()
                .width(Size::px(20.))
                .height(Size::px(32.))
                .child(
                    label()
                        .text("...")
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                )
                .into_element()
        };

        let (start, end) = page_window(current, total_pages, self.window);

        let mut row = rect()
            .horizontal()
            .width(Size::fill())
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .spacing(6.)
            .child(icon_btn(0, current > 0, IconType::ChevronsLeft))
            .child(icon_btn(
                current.saturating_sub(1),
                current > 0,
                IconType::ArrowLeft,
            ));

        if start > 0 {
            row = row.child(num_btn(0));
            if start > 1 {
                row = row.child(ellipsis());
            }
        }

        for p in start..end {
            row = row.child(num_btn(p));
        }

        if end < total_pages {
            if end < last {
                row = row.child(ellipsis());
            }
            row = row.child(num_btn(last));
        }

        row.child(icon_btn(current + 1, current < last, IconType::ArrowRight))
            .child(icon_btn(last, current < last, IconType::ChevronsRight))
    }
}
