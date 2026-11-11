use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use freya::prelude::*;
use oneclient_content::modpacks::SettingFile;

use crate::components::{Button, Icon, IconType, ScrollArea, TextInput};
use crate::i18n::{t, tf};
use crate::theme::{self, colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::utils::count_label;

const LIST_HEIGHT: f32 = 260.;
const ROW_HEIGHT: f32 = 32.;
const ROW_GAP: f32 = 2.;
const FILE_INDENT: f32 = 40.;
const DIR_MAX_WIDTH: f32 = 180.;

#[derive(Clone, Copy, PartialEq)]
pub struct FileChoice {
    pub files: State<Option<Vec<SettingFile>>>,
    pub chosen: State<Option<BTreeSet<String>>>,
    query: State<String>,
    expanded: State<BTreeSet<String>>,
}

impl FileChoice {
    pub fn use_new() -> Self {
        Self {
            files: use_state(|| None::<Vec<SettingFile>>),
            chosen: use_state(|| None::<BTreeSet<String>>),
            query: use_state(String::new),
            expanded: use_state(BTreeSet::new),
        }
    }

    fn all(&self) -> BTreeSet<String> {
        self.files
            .read()
            .iter()
            .flatten()
            .map(|file| file.path.clone())
            .collect()
    }

    #[must_use]
    pub fn is_chosen(&self, path: &str) -> bool {
        self.chosen
            .read()
            .as_ref()
            .is_none_or(|paths| paths.contains(path))
    }

    pub fn toggle(self, path: &str) {
        self.set_many([path.to_string()], !self.is_chosen(path));
    }

    fn set_many(mut self, paths: impl IntoIterator<Item = String>, on: bool) {
        let next = next_choice(&self.all(), self.chosen.read().as_ref(), paths, on);
        self.chosen.set(next);
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
                .filter(|file| paths.contains(&file.path))
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

fn next_choice(
    all: &BTreeSet<String>,
    current: Option<&BTreeSet<String>>,
    paths: impl IntoIterator<Item = String>,
    on: bool,
) -> Option<BTreeSet<String>> {
    let mut chosen = current.cloned().unwrap_or_else(|| all.clone());
    for path in paths {
        if on {
            chosen.insert(path);
        } else {
            chosen.remove(&path);
        }
    }
    (chosen != *all).then_some(chosen)
}

struct Group {
    key: String,
    name: String,
    files: Vec<usize>,
}

#[derive(Clone, Copy)]
enum Row {
    Group(usize),
    File(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Off,
    Some,
    On,
}

fn group_name(path: &str) -> String {
    let mut parts = path.split('/');
    let root = parts.next().unwrap_or_default();
    let rest: Vec<&str> = parts.collect();
    let raw = match rest.as_slice() {
        [] => return "Minecraft".to_string(),
        _ if root == "oneconfig" => return "OneConfig".to_string(),
        [file] => file
            .split(['.', '-', '_'])
            .find(|part| !part.is_empty())
            .unwrap_or(file),
        [dir, ..] => dir,
    };
    let mut chars = raw.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn group_files(files: &[SettingFile], query: &str) -> Vec<Group> {
    let query = query.trim().to_lowercase();
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for (index, file) in files.iter().enumerate() {
        let name = group_name(&file.path);
        let key = name.to_lowercase();
        if !query.is_empty() && !key.contains(&query) && !file.path.to_lowercase().contains(&query)
        {
            continue;
        }
        groups
            .entry(key.clone())
            .or_insert_with(|| Group {
                key,
                name,
                files: Vec::new(),
            })
            .files
            .push(index);
    }
    groups.into_values().collect()
}

pub fn note(text: impl Into<String>, size: f32) -> Element {
    label()
        .text(text.into())
        .font_size(text_size(size))
        .color(colors::fg_secondary())
        .into_element()
}

pub fn settings_file_picker(choice: FileChoice, fill: bool) -> Vec<Element> {
    let files = match choice.files.read().clone() {
        None => return vec![note(t("oneclient.components.settings_files.loading"), 12.5)],
        Some(files) if files.is_empty() => {
            return vec![note(t("oneclient.components.settings_files.empty"), 12.5)];
        }
        Some(files) => files,
    };
    let search = TextInput::new(choice.query)
        .width(Size::fill())
        .placeholder(t("oneclient.components.settings_files.search"))
        .leading(
            Icon::new(IconType::SearchMd)
                .size(16.)
                .color(colors::fg_secondary())
                .into_element(),
        )
        .into_element();

    vec![
        search,
        file_choice_header(choice),
        file_list(choice, files, fill),
    ]
}

pub fn file_choice_header(choice: FileChoice) -> Element {
    let (count, total) = choice.counts();
    let mut chosen = choice.chosen;

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new(0., 0., 0., 8.))
        .child(
            label()
                .text(tf(
                    "oneclient.components.settings_files.chosen",
                    &[&count, &total],
                ))
                .width(Size::flex(1.))
                .font_size(text_size(13.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            Button::new()
                .ghost()
                .small()
                .on_press(move |_| chosen.set(Some(BTreeSet::new())))
                .text(t("oneclient.components.settings_files.select_none")),
        )
        .child(
            Button::new()
                .primary()
                .small()
                .on_press(move |_| chosen.set(None))
                .text(t("oneclient.components.settings_files.select_all")),
        )
        .into_element()
}

fn file_list(choice: FileChoice, files: Vec<SettingFile>, fill: bool) -> Element {
    let query = choice.query.read().clone();
    let searching = !query.trim().is_empty();
    let groups = group_files(&files, &query);
    if groups.is_empty() {
        return note(t("oneclient.components.settings_files.no_match"), 12.5);
    }
    let expanded = choice.expanded.read().clone();
    let mut rows = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        rows.push(Row::Group(index));
        if searching || expanded.contains(&group.key) {
            rows.extend(group.files.iter().map(|&file| Row::File(file)));
        }
    }

    let files = Rc::new(files);
    let groups = Rc::new(groups);
    let rows = Rc::new(rows);
    let count = rows.len();
    let scroll = ScrollArea::new().width(Size::fill());
    let scroll = if fill {
        scroll.height(Size::fill())
    } else {
        scroll.height(Size::Inner).max_height(Size::px(LIST_HEIGHT))
    };
    rect()
        .width(Size::fill())
        .height(if fill { Size::flex(1.) } else { Size::Inner })
        .corner_radius(rounded(10.))
        .background(colors::page())
        .border(border_all_color(1., colors::component_border()))
        .overflow(Overflow::Clip)
        .child(
            scroll
                .padding(Gaps::new_all(4.))
                .lazy(count, ROW_HEIGHT, ROW_GAP, move |i| match rows[i] {
                    Row::Group(group) => {
                        let group = &groups[group];
                        group_row(
                            group,
                            &files,
                            choice,
                            searching || expanded.contains(&group.key),
                        )
                    }
                    Row::File(file) => file_row(&files[file], choice),
                }),
        )
        .into_element()
}

fn tick(mark: Mark) -> Rect {
    let tick = rect()
        .width(Size::px(16.))
        .height(Size::px(16.))
        .center()
        .corner_radius(rounded(4.));
    match mark {
        Mark::Off => tick.border(border_all_color(1., colors::control_border())),
        Mark::Some | Mark::On => tick.background(colors::brand()).child(
            Icon::new(if mark == Mark::On {
                IconType::Check
            } else {
                IconType::Minus
            })
            .size(12.)
            .color(colors::on_brand()),
        ),
    }
}

fn group_row(group: &Group, files: &[SettingFile], choice: FileChoice, expanded: bool) -> Element {
    let total = group.files.len();
    let picked = group
        .files
        .iter()
        .filter(|&&file| choice.is_chosen(&files[file].path))
        .count();
    let mark = match picked {
        0 => Mark::Off,
        n if n == total => Mark::On,
        _ => Mark::Some,
    };
    let count = if picked == total {
        count_label(
            total,
            t("oneclient.components.settings_files.group.one"),
            t("oneclient.components.settings_files.group.other"),
        )
    } else {
        tf(
            "oneclient.components.settings_files.group.some",
            &[&picked, &total],
        )
    };
    let paths: Vec<String> = group
        .files
        .iter()
        .map(|&file| files[file].path.clone())
        .collect();
    let key = group.key.clone();
    let mut open = choice.expanded;

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(8.)
        .padding(Gaps::new(0., 10., 0., 8.))
        .corner_radius(rounded(8.))
        .maybe(expanded, |row| row.background(colors::component_bg()))
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| {
            let mut keys = open.read().clone();
            if !keys.remove(&key) {
                keys.insert(key.clone());
            }
            open.set(keys);
        })
        .child(tick(mark).on_press(move |e: Event<PressEventData>| {
            e.stop_propagation();
            choice.set_many(paths.clone(), mark != Mark::On);
        }))
        .child(
            Icon::new(if expanded {
                IconType::ChevronDown
            } else {
                IconType::ChevronRight
            })
            .size(14.)
            .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(group.name.clone())
                .width(Size::flex(1.))
                .font_size(text_size(13.5))
                .font_weight(FontWeight::SEMI_BOLD)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(count)
                .font_size(text_size(12.5))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn file_row(file: &SettingFile, choice: FileChoice) -> Element {
    let (dir, name) = file
        .path
        .rsplit_once('/')
        .map_or(("", file.path.as_str()), |(dir, name)| (dir, name));
    let selected = choice.is_chosen(&file.path);
    let toggled = file.path.clone();

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(10.)
        .padding(Gaps::new(0., 10., 0., FILE_INDENT))
        .corner_radius(rounded(8.))
        .cursor(CursorIcon::Pointer)
        .on_press(move |_| choice.toggle(&toggled))
        .child(tick(if selected { Mark::On } else { Mark::Off }))
        .child(
            label()
                .text(name.to_string())
                .width(Size::flex(1.))
                .font_size(text_size(13.))
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(if selected {
                    colors::fg_primary()
                } else {
                    colors::fg_secondary()
                }),
        )
        .maybe_child((!dir.is_empty()).then(|| {
            label()
                .text(format!("{dir}/"))
                .max_width(Size::px(DIR_MAX_WIDTH))
                .font_size(text_size(11.5))
                .font_family(theme::mono_font())
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(colors::fg_secondary())
                .into_element()
        }))
        .into_element()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{group_name, next_choice};

    #[test]
    fn groups_settings_by_mod() {
        assert_eq!(group_name("options.txt"), "Minecraft");
        assert_eq!(
            group_name("config/NoChatReports/NCR-Client.json"),
            "NoChatReports"
        );
        assert_eq!(group_name("config/sodium-options.json"), "Sodium");
        assert_eq!(group_name("config/.hidden"), "Hidden");
        assert_eq!(group_name("oneconfig/profiles/a.json"), "OneConfig");
    }

    #[test]
    fn unticked_files_stay_left_out() {
        let set = |paths: &[&str]| -> BTreeSet<String> {
            paths.iter().map(|path| path.to_string()).collect()
        };
        let all = set(&["a", "b", "c"]);

        let without_a = next_choice(&all, None, ["a".to_string()], false);
        assert_eq!(without_a, Some(set(&["b", "c"])));
        assert_eq!(
            next_choice(&all, without_a.as_ref(), ["a".to_string()], true),
            None
        );

        let none = set(&[]);
        assert_eq!(
            next_choice(&all, Some(&none), ["b".to_string()], true),
            Some(set(&["b"]))
        );
        assert_eq!(
            next_choice(&all, None, all.iter().cloned(), false),
            Some(none)
        );
    }
}
