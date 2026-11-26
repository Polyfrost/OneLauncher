use std::collections::HashMap;

use crate::components::{FileChoice, OverlayPopup};
use crate::hooks::{load_setting_files, load_sync_left_out, use_has_game_options};

use super::*;

const SYNC_FILES_WIDTH: f32 = 560.;
const GAME_OPTIONS_WIDTH: f32 = 640.;
const GAME_OPTIONS_LIST_HEIGHT: f32 = 420.;
const GAME_OPTIONS_ROW_HEIGHT: f32 = 30.;
const GAME_OPTIONS_INPUT_HEIGHT: f32 = 28.;

#[derive(PartialEq)]
pub(super) struct GameOptionsRow {
    pub(super) cluster_id: i64,
    pub(super) game_dir: PathBuf,
}

impl Component for GameOptionsRow {
    fn render(&self) -> impl IntoElement {
        let game = use_game_snapshot();
        let clusters = settled_or_loading(&use_clusters()).unwrap_or_default();
        let running = game.is_running(self.cluster_id)
            || game.running_clusters().any(|id| {
                clusters
                    .iter()
                    .any(|c| c.id == id && c.game_dir().is_ok_and(|dir| dir == self.game_dir))
            });
        let mut editing = use_state(|| false);

        let dispatch = use_dispatch();
        let game_dir = self.game_dir.clone();
        let has_options = use_has_game_options(game_dir.clone());
        let button = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                Button::new()
                    .small()
                    .ghost()
                    .enabled(has_options)
                    .on_press(move |_| {
                        dispatch.export_game_options(game_dir.clone(), HashMap::new())
                    })
                    .text(t("oneclient.clusters.export.save_file_ellipsis")),
            )
            .child(
                Button::new()
                    .small()
                    .secondary()
                    .enabled(!running)
                    .maybe(!running, |el| el.on_press(move |_| editing.set(true)))
                    .text(t("oneclient.cluster.settings.game_options.button")),
            );

        let row = if running {
            settings_row_disabled(
                IconType::Sliders04,
                t("oneclient.cluster.settings.game_options.title"),
                t("oneclient.cluster.settings.game_options.locked_running"),
                button,
            )
            .into_element()
        } else {
            settings_row(
                IconType::Sliders04,
                t("oneclient.cluster.settings.game_options.title"),
                t("oneclient.cluster.settings.game_options.description"),
                button,
            )
            .into_element()
        };

        rect()
            .width(Size::fill())
            .child(row)
            .maybe_child((*editing.read()).then(|| {
                GameOptionsModal {
                    game_dir: self.game_dir.clone(),
                    running,
                    on_close: (move |()| editing.set(false)).into(),
                }
                .into_element()
            }))
            .into_element()
    }
}

#[derive(PartialEq)]
struct GameOptionsModal {
    game_dir: PathBuf,
    running: bool,
    on_close: EventHandler<()>,
}

impl Component for GameOptionsModal {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut options = use_state(|| None::<Option<Vec<(String, String)>>>);
        let edits = use_state(HashMap::<String, String>::new);
        let search = use_state(String::new);
        {
            let game_dir = self.game_dir.clone();
            use_hook(move || {
                spawn(async move {
                    options.set(Some(oneclient_core::game::read_options(&game_dir).await));
                });
            });
        }

        let close = self.on_close.clone();
        let cancel = self.on_close.clone();
        let save = self.on_close.clone();
        let game_dir = self.game_dir.clone();
        let loaded = matches!(&*options.read(), Some(Some(_)));
        let changes = move || match &*options.peek() {
            Some(Some(entries)) => oneclient_core::game::changed_options(entries, &edits.peek()),
            _ => HashMap::new(),
        };

        let query = search.read().to_lowercase();
        let body = match &*options.read() {
            None => centered_note(t("oneclient.cluster.settings.loading")).into_element(),
            Some(None) => {
                centered_note(t("oneclient.cluster.settings.game_options.missing")).into_element()
            }
            Some(Some(entries)) => {
                let current = edits.peek();
                let rows: Vec<(String, String)> = entries
                    .iter()
                    .filter(|(name, value)| {
                        let value = current.get(name).unwrap_or(value);
                        name.to_lowercase().contains(&query)
                            || value.to_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect();
                ScrollArea::new()
                    .width(Size::fill())
                    .height(Size::px(GAME_OPTIONS_LIST_HEIGHT))
                    .max_height(Size::window_percent(60.))
                    .scrollbar_gutter(true)
                    .lazy(rows.len(), GAME_OPTIONS_ROW_HEIGHT, 4., move |index| {
                        let (name, value) = &rows[index];
                        let initial = edits.peek().get(name).unwrap_or(value).clone();
                        rect()
                            .key(name.clone())
                            .width(Size::fill())
                            .height(Size::fill())
                            .child(GameOptionField {
                                name: name.clone(),
                                boolean: oneclient_core::game::is_boolean_option(name, value),
                                initial,
                                edits,
                            })
                            .into_element()
                    })
                    .into_element()
            }
        };

        let content = vec![
            label()
                .text(t("oneclient.cluster.settings.game_options.title"))
                .font_size(text_size(18.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary())
                .into_element(),
            label()
                .text(t(
                    "oneclient.cluster.settings.game_options.modal_description",
                ))
                .font_size(text_size(13.))
                .color(colors::fg_secondary())
                .into_element(),
            TextInput::new(search)
                .placeholder(t("oneclient.cluster.settings.game_options.search"))
                .width(Size::fill())
                .into_element(),
            body,
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .spacing(8.)
                .child(
                    Button::new()
                        .secondary()
                        .enabled(loaded)
                        .on_press({
                            let dispatch = dispatch.clone();
                            let game_dir = game_dir.clone();
                            move |_| dispatch.export_game_options(game_dir.clone(), changes())
                        })
                        .text(t("oneclient.clusters.export.save_file_ellipsis")),
                )
                .child(rect().width(Size::flex(1.)))
                .child(
                    Button::new()
                        .ghost()
                        .on_press(move |_| cancel.call(()))
                        .text(t("oneclient.common.cancel")),
                )
                .child(
                    Button::new()
                        .enabled(loaded && !self.running)
                        .on_press(move |_| {
                            dispatch.save_game_options(game_dir.clone(), changes());
                            save.call(());
                        })
                        .text(t("oneclient.common.save")),
                )
                .into_element(),
        ];

        OverlayPopup::new()
            .on_close(move |()| close.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(GAME_OPTIONS_WIDTH))
                            .max_width(Size::window_percent(92.))
                            .spacing(16.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(rounded(16.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .children(content),
                    ),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct GameOptionField {
    name: String,
    boolean: bool,
    initial: String,
    edits: State<HashMap<String, String>>,
}

impl Component for GameOptionField {
    fn render(&self) -> impl IntoElement {
        let mut value = use_state({
            let initial = self.initial.clone();
            move || initial
        });
        {
            let name = self.name.clone();
            let mut edits = self.edits;
            use_side_effect(move || {
                let next = value.read().clone();
                edits.write().insert(name.clone(), next);
            });
        }

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .cross_align(Alignment::Center)
            .spacing(12.)
            .child(
                label()
                    .text(self.name.clone())
                    .width(Size::percent(45.))
                    .font_size(text_size(13.))
                    .font_family(theme::mono_font())
                    .color(colors::fg_primary())
                    .max_lines(1)
                    .text_overflow(TextOverflow::Ellipsis),
            )
            .child(if self.boolean {
                let current = value.read().clone();
                let side = |label: &'static str| SwitchSide {
                    label,
                    active: current == label,
                    accent: true,
                    on_press: EventHandler::new(move |()| value.set(label.to_string())),
                };
                pill_switch([side("false"), side("true")])
                    .height(Size::px(GAME_OPTIONS_INPUT_HEIGHT))
                    .into_element()
            } else {
                TextInput::new(value)
                    .compact()
                    .font_family(theme::mono_font())
                    .width(Size::fill())
                    .height(Size::px(GAME_OPTIONS_INPUT_HEIGHT))
                    .into_element()
            })
            .into_element()
    }
}

#[derive(PartialEq)]
pub(super) struct SyncFilesModal {
    pub(super) cluster_id: i64,
    pub(super) on_close: EventHandler<()>,
}

impl Component for SyncFilesModal {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let dispatch = use_dispatch();
        let choice = FileChoice::use_new();
        use_hook(move || {
            let mut files = choice.files;
            spawn(async move {
                let listed = load_setting_files(cluster_id).await;
                let left_out = load_sync_left_out(cluster_id).await;
                files.set(Some(listed));
                choice.leave_out(&left_out);
            });
        });

        let close = self.on_close.clone();
        let cancel = self.on_close.clone();
        let save = self.on_close.clone();
        let loaded = choice.files.read().is_some();

        let mut content = vec![
            label()
                .text(t("oneclient.cluster.settings.sync_files.title"))
                .font_size(text_size(18.))
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary())
                .into_element(),
            label()
                .text(t("oneclient.cluster.settings.sync_files.description"))
                .font_size(text_size(13.))
                .color(colors::fg_secondary())
                .into_element(),
        ];
        content.extend(settings_file_picker(choice, false));
        content.push(
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::End)
                .spacing(8.)
                .child(
                    Button::new()
                        .ghost()
                        .on_press(move |_| cancel.call(()))
                        .text(t("oneclient.common.cancel")),
                )
                .child(
                    Button::new()
                        .enabled(loaded)
                        .on_press(move |_| {
                            dispatch.set_sync_left_out(cluster_id, choice.left_out());
                            save.call(());
                        })
                        .text(t("oneclient.common.save")),
                )
                .into_element(),
        );

        OverlayPopup::new()
            .on_close(move |()| close.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(SYNC_FILES_WIDTH))
                            .max_width(Size::window_percent(92.))
                            .spacing(16.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(rounded(16.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .children(content),
                    ),
            )
            .into_element()
    }
}
