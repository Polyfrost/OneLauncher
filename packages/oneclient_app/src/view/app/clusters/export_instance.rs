use std::collections::BTreeSet;
use std::time::Duration;

use freya::animation::{
    AnimNum, Ease, Function, OnChange, OnCreation, use_animation_with_dependencies,
};
use freya::prelude::*;
use oneclient_cluster::Cluster;
use oneclient_common::domain::{ContentType, GameLoader};
use oneclient_core::clusters::{ExportMode, ExportOptions, SettingsChoice};
use oneclient_core::sharing::{Collaborator, LinkedPush, MAX_SHARE_BYTES};

use super::instance::{WideCard, wide_card};
use crate::components::{
    Button, FileChoice, Icon, IconType, OverlayPopup, Spinner, TextInput, note,
    settings_file_picker,
};
use crate::hooks::{
    add_collaborator, cluster_content_items, load_collaborators, load_owner_check,
    load_setting_files, measure_share, remove_collaborator, use_cluster, use_cluster_content,
    use_debounced, use_dispatch, use_share_progress,
};
use crate::i18n::{t, tf};
use crate::state::{OwnerCheck, OwnerShare, ShareProgress, ShareReady};
use crate::theme::{self, colors, rounded, text_size};
use crate::ui::{border_all_color, divider};
use crate::utils::count_label;

const DIALOG_WIDTH: f32 = 520.;
const CODE_FONT_SIZE: f32 = 28.;
const TEAM_LIST_MIN_HEIGHT: f32 = 260.;
const TILE_SIZE: f32 = 40.;
const ROW_TILE_SIZE: f32 = 32.;
const MB: f64 = 1024. * 1024.;
const EXPAND_BUTTON_SIZE: f32 = 36.;
const FALLBACK_DIALOG_HEIGHT: f32 = 900.;
const MAX_HEIGHT_PERCENT: f32 = 90.;
const PICKER_HEIGHT: f32 = 430.;
const MEASURE_DELAY: Duration = Duration::from_millis(400);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Copied {
    Code,
    Link,
}

#[derive(Clone, Copy)]
struct Picks {
    mode: State<ExportMode>,
    open: State<bool>,
    files: FileChoice,
    team: Team,
}

#[derive(Clone, Copy)]
struct Team {
    open: State<bool>,
    people: State<Option<Vec<Collaborator>>>,
    username: State<String>,
    message: State<Option<String>>,
    busy: State<bool>,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Measure {
    #[default]
    Running,
    Done(u64),
    Failed,
}

#[derive(Clone, Default)]
struct Summary {
    version: String,
    short_version: String,
    content: String,
    has_content: bool,
    size: Measure,
}

impl Summary {
    fn too_large(&self) -> bool {
        matches!(self.size, Measure::Done(bytes) if bytes > MAX_SHARE_BYTES)
    }

    fn new(cluster: Option<&Cluster>, mods: usize, shaders: usize, packs: usize) -> Self {
        let parts: Vec<String> = [
            (
                mods,
                t("oneclient.clusters.export.summary.mods.one"),
                t("oneclient.clusters.export.summary.mods.other"),
            ),
            (
                shaders,
                t("oneclient.clusters.export.summary.shaders.one"),
                t("oneclient.clusters.export.summary.shaders.other"),
            ),
            (
                packs,
                t("oneclient.clusters.export.summary.resource_packs.one"),
                t("oneclient.clusters.export.summary.resource_packs.other"),
            ),
        ]
        .into_iter()
        .filter(|(count, _, _)| *count > 0)
        .map(|(count, one, other)| count_label(count, one, other))
        .collect();
        let has_content = !parts.is_empty();
        let content = if has_content {
            parts.join(" · ")
        } else {
            t("oneclient.clusters.export.summary.no_content").to_string()
        };
        let Some(cluster) = cluster else {
            return Self {
                content,
                has_content,
                ..Self::default()
            };
        };
        let minecraft = tf(
            "oneclient.clusters.export.summary.minecraft",
            &[&cluster.mc_version],
        );
        let (version, short_version) = match (&cluster.mc_loader, &cluster.mc_loader_version) {
            (GameLoader::Vanilla, _) => (minecraft.clone(), minecraft),
            (loader, Some(loader_version)) => (
                format!("{minecraft} · {loader} {loader_version}"),
                format!("{minecraft} · {loader}"),
            ),
            (loader, None) => {
                let both = format!("{minecraft} · {loader}");
                (both.clone(), both)
            }
        };
        Self {
            version,
            short_version,
            content,
            has_content,
            size: Measure::Running,
        }
    }
}

impl Team {
    fn load(self, code: String) {
        let mut people = self.people;
        let mut message = self.message;
        spawn(async move {
            match load_collaborators(code).await {
                Ok(list) => people.set(Some(list)),
                Err(err) => {
                    people.set(Some(Vec::new()));
                    message.set(Some(err));
                }
            }
        });
    }

    fn add(self, code: String) {
        let name = self.username.read().trim().to_string();
        if name.is_empty() || *self.busy.read() {
            return;
        }
        let (mut people, mut username, mut message, mut busy) =
            (self.people, self.username, self.message, self.busy);
        busy.set(true);
        message.set(None);
        spawn(async move {
            match add_collaborator(code, name).await {
                Ok(added) => {
                    let mut list = people.read().clone().unwrap_or_default();
                    if !list.iter().any(|person| person.uuid == added.uuid) {
                        list.push(added);
                    }
                    people.set(Some(list));
                    username.set(String::new());
                }
                Err(err) => message.set(Some(err)),
            }
            busy.set(false);
        });
    }

    fn remove(self, code: String, person: Collaborator) {
        let (mut people, mut message) = (self.people, self.message);
        spawn(async move {
            match remove_collaborator(code, person.uuid).await {
                Ok(()) => {
                    let mut list = people.read().clone().unwrap_or_default();
                    list.retain(|other| other.uuid != person.uuid);
                    people.set(Some(list));
                }
                Err(err) => message.set(Some(err)),
            }
        });
    }
}

impl Picks {
    fn choice(&self) -> SettingsChoice {
        match &*self.files.chosen.read() {
            None => SettingsChoice::All,
            Some(paths) if paths.is_empty() => SettingsChoice::None,
            Some(paths) => SettingsChoice::Only(paths.clone()),
        }
    }

    fn options(&self) -> ExportOptions {
        ExportOptions::new(*self.mode.read()).with_setting_choice(self.choice())
    }

    fn adopt(mut self, options: &ExportOptions) {
        let mut chosen = self.files.chosen;
        self.mode.set(options.mode);
        match &options.settings {
            SettingsChoice::None => {
                self.open.set(false);
                chosen.set(Some(BTreeSet::new()));
            }
            SettingsChoice::All | SettingsChoice::AllExcept(_) => {
                self.open.set(true);
                chosen.set(None);
            }
            SettingsChoice::Only(paths) => {
                self.open.set(true);
                chosen.set(Some(paths.clone()));
            }
        }
    }
}

fn check_owner(cluster_id: i64, picks: Picks, mut owner: State<OwnerCheck>) {
    spawn(async move {
        let check = load_owner_check(cluster_id).await;
        if let OwnerCheck::Shared(shared) = &check {
            picks.adopt(&shared.options);
        }
        owner.set(check);
    });
}

#[derive(PartialEq)]
pub struct ExportInstanceModal {
    cluster_id: i64,
    name: String,
    on_close: EventHandler<()>,
}

impl ExportInstanceModal {
    pub fn new(cluster_id: i64, name: String, on_close: impl Into<EventHandler<()>>) -> Self {
        Self {
            cluster_id,
            name,
            on_close: on_close.into(),
        }
    }
}

impl Component for ExportInstanceModal {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let team = Team {
            open: use_state(|| false),
            people: use_state(|| None::<Vec<Collaborator>>),
            username: use_state(String::new),
            message: use_state(|| None::<String>),
            busy: use_state(|| false),
        };
        let picks = Picks {
            mode: use_state(|| ExportMode::OneClient),
            open: use_state(|| false),
            files: FileChoice::use_new(),
            team,
        };
        let copied = use_state(|| None::<Copied>);
        let saving = use_state(|| false);
        let file_mode = use_state(|| ExportMode::OneClient);
        let owner = use_state(|| OwnerCheck::Checking);
        use_side_effect(move || {
            if !*team.open.read() || team.people.peek().is_some() {
                return;
            }
            let code = match &*owner.peek() {
                OwnerCheck::Shared(shared) => shared.ready.code.clone(),
                OwnerCheck::Unknown(ready) => ready.code.clone(),
                _ => return,
            };
            team.load(code);
        });
        let share = use_share_progress(self.cluster_id);
        let cluster_id = self.cluster_id;

        let cluster = use_cluster(cluster_id);
        let mods = cluster_content_items(&use_cluster_content(cluster_id, ContentType::Mod)).len();
        let shaders =
            cluster_content_items(&use_cluster_content(cluster_id, ContentType::Shader)).len();
        let packs =
            cluster_content_items(&use_cluster_content(cluster_id, ContentType::ResourcePack))
                .len();
        let mut summary = Summary::new(cluster.as_ref(), mods, shaders, packs);
        let mut size = use_state(|| Measure::Running);
        let mut measuring = use_state(|| None::<TaskHandle>);
        let settled = use_debounced(picks.options(), MEASURE_DELAY);
        use_side_effect_with_deps(&*settled.read(), move |options| {
            if let Some(stale) = measuring.peek().as_ref() {
                stale.cancel();
            }
            size.set(Measure::Running);
            let options = options.clone();
            measuring.set(Some(spawn(async move {
                let result = measure_share(cluster_id, options).await;
                size.set(result.map_or(Measure::Failed, Measure::Done));
            })));
        });
        summary.size = *size.read();
        let fit_height = use_state(|| None::<f32>);

        use_hook(move || {
            let mut chosen = picks.files.chosen;
            chosen.set(Some(BTreeSet::new()));
            let mut files = picks.files.files;
            spawn(async move { files.set(Some(load_setting_files(cluster_id).await)) });
            check_owner(cluster_id, picks, owner);
        });

        let on_close = self.on_close.clone();
        let close: EventHandler<()> = {
            let dispatch = dispatch.clone();
            (move |()| {
                dispatch.dismiss_share(cluster_id);
                on_close.call(());
            })
            .into()
        };

        let fitting = share.is_none() && !*team.open.read();
        let fill = fitting && *picks.open.read();
        let (body, footer) = match share {
            None if *team.open.read() => team_view(team, &owner.read()),
            None if *saving.read() => save_view(
                &self.name,
                cluster_id,
                picks,
                file_mode,
                saving,
                &dispatch,
                close.clone(),
            ),
            None => options_view(
                &self.name,
                cluster_id,
                picks,
                &summary,
                &owner.read(),
                copied,
                saving,
                &dispatch,
                close.clone(),
            ),
            Some(progress) => share_view(
                &self.name,
                cluster_id,
                progress,
                picks,
                &summary,
                owner,
                copied,
                &dispatch,
                close.clone(),
            ),
        };

        let mut content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(16.)
            .padding(Gaps::new_all(20.))
            .children(body);
        let mut dialog = rect()
            .vertical()
            .width(Size::px(DIALOG_WIDTH))
            .max_width(Size::window_percent(92.))
            .corner_radius(rounded(16.))
            .background(colors::page_elevated())
            .border(border_all_color(1., colors::component_border()))
            .overflow(Overflow::Clip);
        if fill {
            let desired = fit_height
                .read()
                .map_or(FALLBACK_DIALOG_HEIGHT, |height| height + PICKER_HEIGHT);
            content = content.height(Size::flex(1.)).content(Content::Flex);
            dialog = dialog.content(Content::Flex).height(Size::func_data(
                move |ctx| Some(desired.min(ctx.root * MAX_HEIGHT_PERCENT / 100.)),
                &desired.to_bits(),
            ));
        } else {
            let mut fit_height = fit_height;
            dialog = dialog
                .max_height(Size::window_percent(MAX_HEIGHT_PERCENT))
                .on_sized(move |e: Event<SizedEventData>| {
                    if fitting {
                        fit_height.set_if_modified(Some(e.area.height()));
                    }
                });
        }

        let close_scrim = close.clone();
        OverlayPopup::new()
            .on_close(move |()| close_scrim.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        dialog.child(content).child(divider()).child(
                            rect()
                                .width(Size::fill())
                                .padding(Gaps::new(16., 20., 16., 20.))
                                .child(footer),
                        ),
                    ),
            )
            .into_element()
    }
}

fn title(text: String) -> Element {
    label()
        .text(text)
        .font_size(text_size(22.))
        .font_weight(FontWeight::BOLD)
        .color(colors::fg_strong())
        .into_element()
}

fn section_label(text: &str) -> Element {
    label()
        .text(text.to_string())
        .font_size(text_size(13.))
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_secondary())
        .into_element()
}

fn tile(icon: IconType, size: f32, color: Color, background: Color) -> Element {
    rect()
        .width(Size::px(size))
        .height(Size::px(size))
        .center()
        .corner_radius(rounded(size / 4.))
        .background(background)
        .child(Icon::new(icon).size(size / 2.).color(color))
        .into_element()
}

fn card() -> Rect {
    rect()
        .vertical()
        .width(Size::fill())
        .corner_radius(rounded(12.))
        .background(colors::page())
        .border(border_all_color(1., colors::component_border()))
}

fn dialog_header(
    icon: Element,
    heading: String,
    subtitle: Option<&str>,
    extra: Option<Element>,
    close: EventHandler<()>,
) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .spacing(14.)
        .child(icon)
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .spacing(4.)
                .child(title(heading))
                .maybe_child(subtitle.map(|text| note(text, 13.5))),
        )
        .maybe_child(extra)
        .child(
            Button::new()
                .ghost()
                .icon()
                .on_press(move |_| close.call(()))
                .child(
                    Icon::new(IconType::XClose)
                        .size(18.)
                        .color(colors::fg_secondary()),
                ),
        )
        .into_element()
}

fn share_tile() -> Element {
    tile(
        IconType::Share01,
        TILE_SIZE,
        colors::fg_primary(),
        colors::component_bg(),
    )
}

fn footer(left: Option<Element>, right: Vec<Element>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(rect().horizontal().width(Size::flex(1.)).children(left))
        .child(rect().horizontal().spacing(8.).children(right))
        .into_element()
}

fn owner_panel(check: &OwnerCheck, copied: State<Option<Copied>>) -> Option<Element> {
    let (ready, status) = match check {
        OwnerCheck::NotShared => return None,
        OwnerCheck::Checking => {
            return Some(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .padding(Gaps::new(12., 0., 12., 0.))
                    .main_align(Alignment::Center)
                    .cross_align(Alignment::Center)
                    .spacing(8.)
                    .child(Spinner::new(14.))
                    .child(note(t("oneclient.clusters.export.checking"), 13.))
                    .into_element(),
            );
        }
        OwnerCheck::Unknown(ready) => (
            ready,
            status_line(
                IconType::RefreshCw01,
                colors::fg_secondary(),
                t("oneclient.clusters.export.status.unknown"),
            ),
        ),
        OwnerCheck::Shared(shared) => (&shared.ready, change_line(shared)),
        OwnerCheck::Collaborator(ready) => (
            ready,
            status_line(
                IconType::Users01,
                colors::brand(),
                t("oneclient.clusters.export.status.collaborator"),
            ),
        ),
    };

    Some(
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(code_box(ready, copied))
            .child(status)
            .into_element(),
    )
}

fn code_box(ready: &ShareReady, copied: State<Option<Copied>>) -> Element {
    let mut code = rect().vertical().width(Size::flex(1.)).spacing(4.).child(
        label()
            .text(ready.code.clone())
            .font_size(text_size(CODE_FONT_SIZE))
            .font_family(theme::mono_font())
            .font_weight(FontWeight::BOLD)
            .max_lines(1)
            .color(colors::fg_strong()),
    );
    if let Some(expires) = &ready.expires {
        code = code.child(expiry(expires, 12.5));
    }

    card()
        .horizontal()
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(6.)
        .padding(Gaps::new(14., 14., 14., 20.))
        .child(code)
        .child(copy_icon(
            ready.link.clone(),
            Copied::Link,
            copied,
            IconType::Link03,
            t("oneclient.clusters.export.copy_link"),
        ))
        .child(copy_code_button(ready.code.clone(), copied))
        .into_element()
}

fn copy_code_button(code: String, mut copied: State<Option<Copied>>) -> Element {
    let done = *copied.read() == Some(Copied::Code);
    Button::new()
        .primary()
        .on_press(move |_| {
            if freya::text_edit::Clipboard::set(code.clone()).is_ok() {
                copied.set(Some(Copied::Code));
            }
        })
        .child(
            Icon::new(if done {
                IconType::Check
            } else {
                IconType::Copy01
            })
            .size(16.),
        )
        .text(if done {
            t("oneclient.clusters.export.copied")
        } else {
            t("oneclient.clusters.export.copy")
        })
        .into_element()
}

fn copy_icon(
    text: String,
    which: Copied,
    mut copied: State<Option<Copied>>,
    icon: IconType,
    tooltip: &'static str,
) -> Element {
    let done = *copied.read() == Some(which);
    Button::new()
        .ghost()
        .icon()
        .tooltip(tooltip)
        .on_press(move |_| {
            if freya::text_edit::Clipboard::set(text.clone()).is_ok() {
                copied.set(Some(which));
            }
        })
        .child(
            Icon::new(if done { IconType::Check } else { icon })
                .size(18.)
                .color(if done {
                    colors::success()
                } else {
                    colors::fg_secondary()
                }),
        )
        .into_element()
}

fn expiry(date: &str, size: f32) -> Element {
    let template = t("oneclient.clusters.export.works_until");
    let (before, after) = template.split_once("%s").unwrap_or((template, ""));
    paragraph()
        .font_size(text_size(size))
        .span(Span::new(before).color(colors::fg_secondary()))
        .span(
            Span::new(date.to_string())
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .span(Span::new(after).color(colors::fg_secondary()))
        .into_element()
}

fn change_line(shared: &OwnerShare) -> Element {
    if shared.behind {
        return status_line(
            IconType::AlertTriangle,
            colors::code_warn(),
            t("oneclient.clusters.export.status.behind"),
        );
    }
    match (shared.content_changed, shared.settings_changed) {
        (false, false) => status_line(
            IconType::CheckCircle,
            colors::success(),
            t("oneclient.clusters.export.status.current"),
        ),
        (true, false) => status_line(
            IconType::RefreshCw01,
            colors::brand(),
            t("oneclient.clusters.export.status.mods_changed"),
        ),
        (false, true) => status_line(
            IconType::RefreshCw01,
            colors::brand(),
            t("oneclient.clusters.export.status.settings_changed"),
        ),
        (true, true) => status_line(
            IconType::RefreshCw01,
            colors::brand(),
            t("oneclient.clusters.export.status.both_changed"),
        ),
    }
}

fn summary_row(icon: IconType, name: &str, value: String, danger: bool) -> Element {
    let (accent, tile_bg) = if danger {
        (colors::danger(), colors::with_alpha(colors::danger(), 32))
    } else {
        (colors::fg_primary(), colors::component_bg())
    };
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(tile(icon, ROW_TILE_SIZE, accent, tile_bg))
        .child(
            label()
                .text(name.to_string())
                .font_size(text_size(13.5))
                .color(if danger {
                    colors::danger()
                } else {
                    colors::fg_secondary()
                }),
        )
        .child(
            label()
                .text(value)
                .width(Size::flex(1.))
                .text_align(TextAlign::End)
                .font_size(text_size(14.))
                .font_weight(FontWeight::MEDIUM)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(accent),
        )
        .into_element()
}

fn size_label(size: Measure) -> String {
    let limit = MAX_SHARE_BYTES as f64 / MB;
    match size {
        Measure::Done(bytes) => format!("{:.2} / {limit:.1} MB", bytes as f64 / MB),
        Measure::Running => format!("… / {limit:.1} MB"),
        Measure::Failed => format!("— / {limit:.1} MB"),
    }
}

fn fitted_settings(summary: &Summary, picks: Picks) -> Element {
    if *picks.open.read() {
        return rect()
            .vertical()
            .width(Size::fill())
            .height(Size::flex(1.))
            .content(Content::Flex)
            .spacing(12.)
            .child(settings_card(picks, true))
            .maybe_child(summary.too_large().then(|| {
                status_line(
                    IconType::AlertTriangle,
                    colors::danger(),
                    &tf(
                        "oneclient.clusters.export.too_large",
                        &[&size_label(summary.size)],
                    ),
                )
            }))
            .into_element();
    }
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(16.)
        .child(summary_card(summary))
        .child(settings_card(picks, false))
        .into_element()
}

fn summary_card(summary: &Summary) -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .child(section_label(t("oneclient.clusters.export.summary.title")))
        .child(
            card()
                .spacing(16.)
                .padding(Gaps::new(14., 14., 14., 14.))
                .child(summary_row(
                    IconType::Cube01,
                    t("oneclient.clusters.export.summary.version"),
                    summary.version.clone(),
                    false,
                ))
                .child(summary_row(
                    IconType::LayersThree01,
                    t("oneclient.clusters.export.summary.content"),
                    summary.content.clone(),
                    false,
                ))
                .child(summary_row(
                    IconType::Database01,
                    t("oneclient.clusters.export.summary.size"),
                    size_label(summary.size),
                    summary.too_large(),
                )),
        )
        .into_element()
}

fn settings_card(picks: Picks, fill: bool) -> Element {
    let expanded = *picks.open.read();
    let header = rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(12.)
        .padding(Gaps::new(14., 14., 14., 14.))
        .child(tile(
            IconType::Sliders04,
            ROW_TILE_SIZE,
            colors::fg_primary(),
            colors::component_bg(),
        ))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.))
                .spacing(2.)
                .child(
                    label()
                        .text(t("oneclient.clusters.export.settings.include"))
                        .font_size(text_size(14.5))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_strong()),
                )
                .child(note(t("oneclient.clusters.export.settings.subtitle"), 13.)),
        )
        .child(ExpandButton { open: picks.open });

    let mut card = card().child(header);
    if fill {
        card = card.height(Size::flex(1.)).content(Content::Flex);
    }
    if expanded {
        let mut picker = rect()
            .vertical()
            .width(Size::fill())
            .spacing(12.)
            .padding(Gaps::new(12., 10., 14., 10.));
        if fill {
            picker = picker.height(Size::flex(1.)).content(Content::Flex);
        }
        card = card.child(divider()).child(
            picker
                .children(settings_file_picker(picks.files, fill))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .spacing(8.)
                        .padding(Gaps::new(0., 6., 0., 6.))
                        .child(
                            Icon::new(IconType::Lock01)
                                .size(14.)
                                .color(colors::fg_secondary()),
                        )
                        .child(
                            label()
                                .text(t("oneclient.clusters.export.settings.private"))
                                .width(Size::flex(1.))
                                .font_size(text_size(12.5))
                                .color(colors::fg_secondary()),
                        ),
                ),
        );
    }
    card.into_element()
}

fn team_button(owner: &OwnerCheck, team: Team) -> Option<Element> {
    let owns = matches!(owner, OwnerCheck::Shared(_) | OwnerCheck::Unknown(_));
    let (mut open, mut people, mut message) = (team.open, team.people, team.message);
    owns.then(|| {
        Button::new()
            .ghost()
            .icon()
            .tooltip(t("oneclient.clusters.export.team.title"))
            .on_press(move |_| {
                people.set(None);
                message.set(None);
                open.set(true);
            })
            .child(
                Icon::new(IconType::Users01)
                    .size(18.)
                    .color(colors::fg_secondary()),
            )
            .into_element()
    })
}

#[allow(clippy::too_many_arguments)]
fn options_view(
    name: &str,
    cluster_id: i64,
    picks: Picks,
    summary: &Summary,
    owner: &OwnerCheck,
    copied: State<Option<Copied>>,
    mut saving: State<bool>,
    dispatch: &crate::hooks::Actions,
    close: EventHandler<()>,
) -> (Vec<Element>, Element) {
    let shared_ready = match owner {
        OwnerCheck::Shared(shared) => Some(shared.ready.clone()),
        OwnerCheck::Unknown(ready) | OwnerCheck::Collaborator(ready) => Some(ready.clone()),
        OwnerCheck::Checking | OwnerCheck::NotShared => None,
    };
    let collaborating = matches!(owner, OwnerCheck::Collaborator(_));
    let checking = matches!(owner, OwnerCheck::Checking);
    let settings_only = matches!(
        owner,
        OwnerCheck::Shared(shared)
            if shared.settings_changed && !shared.content_changed && !shared.behind
    );

    let mut body = vec![dialog_header(
        share_tile(),
        tf("oneclient.clusters.export.share_title", &[&name]),
        Some(t("oneclient.clusters.export.subtitle")),
        team_button(owner, picks.team),
        close,
    )];
    body.extend(owner_panel(owner, copied));
    body.push(fitted_settings(summary, picks));

    let share_dispatch = dispatch.clone();
    let stop_dispatch = dispatch.clone();
    let right = vec![
        Button::new()
            .secondary()
            .on_press(move |_| saving.set(true))
            .child(Icon::new(IconType::Download01).size(16.))
            .text(t("oneclient.clusters.export.save_file"))
            .into_element(),
        Button::new()
            .primary()
            .enabled(!checking)
            .on_press(move |_| match (collaborating, settings_only) {
                (true, true) => share_dispatch
                    .share_as_collaborator(cluster_id, LinkedPush::SettingsOnly(picks.choice())),
                (true, false) => share_dispatch
                    .share_as_collaborator(cluster_id, LinkedPush::Everything(picks.options())),
                (false, true) => share_dispatch.share_settings_only(cluster_id, picks.choice()),
                (false, false) => share_dispatch.share_cluster(cluster_id, picks.options()),
            })
            .text(match (&shared_ready, settings_only) {
                (None, _) => t("oneclient.clusters.export.get_code"),
                (Some(_), true) => t("oneclient.clusters.export.update_settings"),
                (Some(_), false) => t("oneclient.clusters.export.update_share"),
            })
            .into_element(),
    ];

    let left = shared_ready.filter(|_| !collaborating).map(|ready| {
        Button::new()
            .ghost()
            .foreground(colors::danger())
            .on_press(move |_| stop_dispatch.stop_sharing(cluster_id, ready.clone()))
            .text(t("oneclient.clusters.export.stop"))
            .into_element()
    });
    (body, footer(left, right))
}

fn save_view(
    name: &str,
    cluster_id: i64,
    picks: Picks,
    mut file_mode: State<ExportMode>,
    mut saving: State<bool>,
    dispatch: &crate::hooks::Actions,
    close: EventHandler<()>,
) -> (Vec<Element>, Element) {
    let chosen = *file_mode.read();
    let cards = [
        (
            ExportMode::OneClient,
            IconType::IconLogo,
            "OneClient",
            Some(t("oneclient.clusters.export.oneclient.badge")),
            t("oneclient.clusters.export.oneclient.blurb"),
        ),
        (
            ExportMode::Compatibility,
            IconType::Globe01,
            t("oneclient.clusters.export.compatibility.title"),
            None,
            t("oneclient.clusters.export.compatibility.blurb"),
        ),
    ];

    let export = dispatch.clone();
    let close_export = close.clone();
    let expanded = *picks.open.read();
    let mut body = vec![dialog_header(
        tile(
            IconType::Download01,
            TILE_SIZE,
            colors::fg_primary(),
            colors::component_bg(),
        ),
        tf("oneclient.clusters.export.title", &[&name]),
        None,
        None,
        close,
    )];
    if !expanded {
        body.push(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(10.)
                .children(
                    cards
                        .into_iter()
                        .map(|(option, icon, title, badge, blurb)| {
                            wide_card(WideCard {
                                icon,
                                title: title.to_string(),
                                badge: badge.map(str::to_string),
                                blurb: blurb.to_string(),
                                meta: String::new(),
                                selected: chosen == option,
                                on_press: (move |()| file_mode.set(option)).into(),
                                trailing: None,
                            })
                        }),
                )
                .into_element(),
        );
    }
    body.push(settings_card(picks, expanded));
    let footer = footer(
        Some(back_button(move || saving.set(false))),
        vec![
            Button::new()
                .primary()
                .on_press(move |_| {
                    let options =
                        ExportOptions::new(*file_mode.peek()).with_setting_choice(picks.choice());
                    export.export_cluster(cluster_id, options);
                    close_export.call(());
                })
                .text(t("oneclient.clusters.export.confirm"))
                .into_element(),
        ],
    );
    (body, footer)
}

fn back_button(mut on_back: impl FnMut() + 'static) -> Element {
    Button::new()
        .ghost()
        .on_press(move |_| on_back())
        .child(Icon::new(IconType::ArrowLeft).size(16.))
        .text(t("oneclient.common.back"))
        .into_element()
}

fn team_view(team: Team, owner: &OwnerCheck) -> (Vec<Element>, Element) {
    let code = match owner {
        OwnerCheck::Shared(shared) => shared.ready.code.clone(),
        OwnerCheck::Unknown(ready) => ready.code.clone(),
        _ => String::new(),
    };
    let mut open = team.open;
    let busy = *team.busy.read();

    let people = team.people.read();
    let placeholder = people.as_ref().is_none_or(Vec::is_empty);
    let rows: Vec<Element> = match &*people {
        None => vec![
            rect()
                .horizontal()
                .spacing(8.)
                .cross_align(Alignment::Center)
                .child(Spinner::new(14.))
                .child(note(t("oneclient.clusters.export.team.loading"), 12.5))
                .into_element(),
        ],
        Some(people) if people.is_empty() => vec![
            rect()
                .vertical()
                .spacing(10.)
                .cross_align(Alignment::Center)
                .child(
                    Icon::new(IconType::Users01)
                        .size(28.)
                        .color(colors::fg_secondary()),
                )
                .child(note(t("oneclient.clusters.export.team.empty"), 13.))
                .into_element(),
        ],
        Some(people) => people
            .iter()
            .map(|person| {
                let removed = person.clone();
                let code = code.clone();
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(10.)
                    .padding(Gaps::new(4., 8., 4., 8.))
                    .child(
                        label()
                            .text(
                                person
                                    .username
                                    .clone()
                                    .unwrap_or_else(|| person.uuid.to_string()),
                            )
                            .width(Size::flex(1.))
                            .font_size(text_size(13.))
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        Button::new()
                            .ghost()
                            .foreground(colors::danger())
                            .on_press(move |_| team.remove(code.clone(), removed.clone()))
                            .text(t("oneclient.common.remove")),
                    )
                    .into_element()
            })
            .collect(),
    };
    drop(people);

    let mut list = rect()
        .vertical()
        .width(Size::fill())
        .min_height(Size::px(TEAM_LIST_MIN_HEIGHT))
        .spacing(2.)
        .children(rows);
    if placeholder {
        list = list.center();
    }

    let add_code = code.clone();
    let submit_code = code;
    let mut body = vec![
        title(t("oneclient.clusters.export.team.title").to_string()),
        note(t("oneclient.clusters.export.team.description"), 13.),
        list.into_element(),
        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                rect().width(Size::flex(1.)).child(
                    TextInput::new(team.username)
                        .width(Size::fill())
                        .placeholder(t("oneclient.clusters.export.team.username"))
                        .on_submit(move |_| team.add(submit_code.clone())),
                ),
            )
            .child(
                Button::new()
                    .enabled(!busy)
                    .on_press(move |_| team.add(add_code.clone()))
                    .text(if busy {
                        t("oneclient.clusters.export.team.adding")
                    } else {
                        t("oneclient.common.add")
                    }),
            )
            .into_element(),
    ];
    if let Some(message) = team.message.read().clone() {
        body.push(status_line(
            IconType::AlertTriangle,
            colors::danger(),
            &message,
        ));
    }
    let footer = footer(
        None,
        vec![
            Button::new()
                .primary()
                .on_press(move |_| open.set(false))
                .text(t("oneclient.common.done"))
                .into_element(),
        ],
    );
    (body, footer)
}

#[allow(clippy::too_many_arguments)]
fn share_view(
    name: &str,
    cluster_id: i64,
    progress: ShareProgress,
    picks: Picks,
    summary: &Summary,
    owner: State<OwnerCheck>,
    copied: State<Option<Copied>>,
    dispatch: &crate::hooks::Actions,
    close: EventHandler<()>,
) -> (Vec<Element>, Element) {
    let header = |close| {
        dialog_header(
            share_tile(),
            tf("oneclient.clusters.export.share_title", &[&name]),
            None,
            None,
            close,
        )
    };
    let busy = |text: &str, close: EventHandler<()>| {
        (
            vec![
                header(close.clone()),
                rect()
                    .vertical()
                    .width(Size::fill())
                    .padding(Gaps::new(28., 0., 28., 0.))
                    .center()
                    .spacing(12.)
                    .child(Spinner::new(24.))
                    .child(note(text, 13.))
                    .into_element(),
            ],
            footer(
                None,
                vec![done_button(t("oneclient.clusters.export.hide"), close)],
            ),
        )
    };
    match progress {
        ShareProgress::Working => busy(t("oneclient.clusters.export.uploading"), close),
        ShareProgress::Stopping(_) => busy(t("oneclient.clusters.export.stopping"), close),
        ShareProgress::Ready(ready) => ready_view(
            cluster_id, ready, picks, summary, owner, copied, dispatch, close,
        ),
        ShareProgress::Stopped => (
            vec![
                header(close.clone()),
                status_line(
                    IconType::CheckCircle,
                    colors::success(),
                    t("oneclient.clusters.export.stopped"),
                ),
            ],
            footer(None, vec![done_button(t("oneclient.common.done"), close)]),
        ),
        ShareProgress::Failed(reason) => {
            let retry = dispatch.clone();
            let close_cancel = close.clone();
            (
                vec![
                    header(close),
                    status_line(
                        IconType::AlertTriangle,
                        colors::danger(),
                        t("oneclient.clusters.export.failed"),
                    ),
                    note(reason, 13.),
                ],
                footer(
                    None,
                    vec![
                        Button::new()
                            .ghost()
                            .on_press(move |_| close_cancel.call(()))
                            .text(t("oneclient.common.close"))
                            .into_element(),
                        Button::new()
                            .primary()
                            .on_press(move |_| retry.share_cluster(cluster_id, picks.options()))
                            .text(t("oneclient.common.try_again"))
                            .into_element(),
                    ],
                ),
            )
        }
    }
}

fn chip(text: String) -> Element {
    rect()
        .padding(Gaps::new(6., 12., 6., 12.))
        .corner_radius(rounded(999.))
        .background(colors::component_bg())
        .child(
            label()
                .text(text)
                .font_size(text_size(13.))
                .font_weight(FontWeight::MEDIUM)
                .color(colors::fg_primary()),
        )
        .into_element()
}

#[allow(clippy::too_many_arguments)]
fn ready_view(
    cluster_id: i64,
    ready: ShareReady,
    picks: Picks,
    summary: &Summary,
    owner: State<OwnerCheck>,
    copied: State<Option<Copied>>,
    dispatch: &crate::hooks::Actions,
    close: EventHandler<()>,
) -> (Vec<Element>, Element) {
    let settings = picks.files.counts().0;
    let chips: Vec<Element> = [
        (!summary.short_version.is_empty()).then(|| summary.short_version.clone()),
        summary
            .has_content
            .then(|| t("oneclient.clusters.export.ready.content").to_string()),
        (settings > 0).then(|| {
            count_label(
                settings,
                t("oneclient.clusters.export.ready.settings.one"),
                t("oneclient.clusters.export.ready.settings.other"),
            )
        }),
    ]
    .into_iter()
    .flatten()
    .map(chip)
    .collect();

    let mut body = vec![
        dialog_header(
            tile(
                IconType::CheckCircle,
                TILE_SIZE,
                colors::success(),
                colors::with_alpha(colors::success(), 32),
            ),
            t("oneclient.clusters.export.ready.title").to_string(),
            Some(t("oneclient.clusters.export.ready.subtitle")),
            None,
            close.clone(),
        ),
        code_box(&ready, copied),
    ];
    if ready.incomplete {
        body.push(status_line(
            IconType::AlertTriangle,
            colors::danger(),
            t("oneclient.clusters.export.incomplete"),
        ));
    }
    if !chips.is_empty() {
        body.push(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(10.)
                .child(section_label(t("oneclient.clusters.export.ready.contents")))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::wrap_spacing(8.))
                        .spacing(8.)
                        .children(chips),
                )
                .into_element(),
        );
    }

    let back = dispatch.clone();
    let mut owner = owner;
    let footer = footer(
        Some(back_button(move || {
            back.dismiss_share(cluster_id);
            owner.set(OwnerCheck::Checking);
            check_owner(cluster_id, picks, owner);
        })),
        vec![done_button(t("oneclient.common.done"), close)],
    );
    (body, footer)
}

fn done_button(text: &'static str, close: EventHandler<()>) -> Element {
    Button::new()
        .primary()
        .on_press(move |_| close.call(()))
        .text(text)
        .into_element()
}

fn status_line(icon: IconType, color: Color, text: &str) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(Icon::new(icon).size(16.).color(color))
        .child(
            label()
                .text(text.to_string())
                .width(Size::flex(1.))
                .font_size(text_size(13.))
                .color(colors::fg_primary()),
        )
        .into_element()
}

#[derive(PartialEq)]
struct ExpandButton {
    open: State<bool>,
}

impl Component for ExpandButton {
    fn render(&self) -> impl IntoElement {
        let mut open = self.open;
        let is_open = *open.read();
        let swing = use_animation_with_dependencies(&is_open, |conf, open| {
            conf.on_creation(OnCreation::Finish);
            conf.on_change(OnChange::Rerun);
            let (from, to) = if *open { (0., 180.) } else { (180., 0.) };
            AnimNum::new(from, to)
                .time(180)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        Button::new()
            .secondary()
            .icon()
            .width(Size::px(EXPAND_BUTTON_SIZE))
            .height(Size::px(EXPAND_BUTTON_SIZE))
            .on_press(move |_| open.toggle())
            .child(
                Icon::new(IconType::ChevronDown)
                    .size(18.)
                    .rotate(swing.get().value())
                    .color(colors::fg_primary()),
            )
    }
}
