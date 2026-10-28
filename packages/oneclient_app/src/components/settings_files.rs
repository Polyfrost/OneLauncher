use std::collections::BTreeSet;

use freya::prelude::*;

use crate::components::{Button, Icon, IconType, ScrollArea};
use crate::theme::colors;
use crate::ui::border_all_color;

const LIST_HEIGHT: f32 = 300.;

#[derive(Clone, Copy, PartialEq)]
pub struct FileChoice {
    pub files: State<Option<Vec<String>>>,
    pub chosen: State<Option<BTreeSet<String>>>,
}

impl FileChoice {
    pub fn use_new() -> Self {
        Self {
            files: use_state(|| None::<Vec<String>>),
            chosen: use_state(|| None::<BTreeSet<String>>),
        }
    }

    fn all(&self) -> BTreeSet<String> {
        self.files.read().iter().flatten().cloned().collect()
    }

    #[must_use]
    pub fn is_chosen(&self, path: &str) -> bool {
        self.chosen
            .read()
            .as_ref()
            .is_none_or(|paths| paths.contains(path))
    }

    pub fn toggle(mut self, path: &str) {
        let all = self.all();
        let mut paths = self.chosen.read().clone().unwrap_or_else(|| all.clone());
        if !paths.remove(path) {
            paths.insert(path.to_string());
        }
        self.chosen
            .set(if paths == all { None } else { Some(paths) });
    }

    #[must_use]
    pub fn counts(&self) -> (usize, usize) {
        let files = self.files.read();
        let total = files.as_ref().map_or(0, Vec::len);
        let chosen = match &*self.chosen.read() {
            None => total,
            Some(paths) => files
                .iter()
                .flatten()
                .filter(|path| paths.contains(*path))
                .count(),
        };
        (chosen, total)
    }

    #[must_use]
    pub fn left_out(&self) -> BTreeSet<String> {
        match &*self.chosen.read() {
            None => BTreeSet::new(),
            Some(paths) => self.all().difference(paths).cloned().collect(),
        }
    }

    pub fn leave_out(mut self, left_out: &BTreeSet<String>) {
        if left_out.is_empty() {
            self.chosen.set(None);
            return;
        }
        let chosen: BTreeSet<String> = self.all().difference(left_out).cloned().collect();
        self.chosen.set(Some(chosen));
    }
}

pub fn settings_file_picker(choice: FileChoice) -> Vec<Element> {
    let mut chosen = choice.chosen;
    let (count, total) = choice.counts();
    let files = choice.files.read().clone();

    let list: Element = match files {
        None => label()
            .text("Looking for settings files…")
            .font_size(12.5)
            .color(colors::fg_secondary())
            .into_element(),
        Some(files) if files.is_empty() => label()
            .text("This instance has no settings files yet.")
            .font_size(12.5)
            .color(colors::fg_secondary())
            .into_element(),
        Some(files) => ScrollArea::new()
            .width(Size::fill())
            .height(Size::Inner)
            .max_height(Size::px(LIST_HEIGHT))
            .child(
                rect().vertical().width(Size::fill()).spacing(2.).children(
                    files
                        .iter()
                        .map(|path| file_row(path.clone(), choice.is_chosen(path), choice)),
                ),
            )
            .into_element(),
    };

    vec![
        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                rect().width(Size::flex(1.)).child(
                    label()
                        .text(format!("{count} of {total} chosen"))
                        .font_size(12.5)
                        .color(colors::fg_secondary()),
                ),
            )
            .child(
                Button::new()
                    .ghost()
                    .on_press(move |_| chosen.set(None))
                    .text("Select all"),
            )
            .child(
                Button::new()
                    .ghost()
                    .on_press(move |_| chosen.set(Some(BTreeSet::new())))
                    .text("Select none"),
            )
            .into_element(),
        list,
    ]
}

fn file_row(path: String, selected: bool, choice: FileChoice) -> Element {
    let toggled = path.clone();
    let mut tick = rect()
        .width(Size::px(16.))
        .height(Size::px(16.))
        .center()
        .corner_radius(CornerRadius::new_all(4.));
    tick = if selected {
        tick.background(colors::brand()).child(
            Icon::new(IconType::Check)
                .size(12.)
                .color(colors::fg_strong()),
        )
    } else {
        tick.border(border_all_color(1., colors::control_border()))
    };
    rect()
        .horizontal()
        .width(Size::fill())
        .spacing(10.)
        .padding(Gaps::new(6., 8., 6., 8.))
        .cross_align(Alignment::Center)
        .corner_radius(CornerRadius::new_all(8.))
        .on_press(move |_| choice.toggle(&toggled))
        .child(tick)
        .child(
            label()
                .text(path)
                .font_size(12.5)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(colors::fg_primary()),
        )
        .into_element()
}
