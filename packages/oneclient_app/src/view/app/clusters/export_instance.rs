use freya::prelude::*;
use oneclient_core::clusters::{ExportMode, ExportOptions, SettingsChoice};
use oneclient_core::sharing::{Collaborator, LinkedPush};

use super::instance::{WideCard, wide_card};
use crate::components::{
    Button, FileChoice, Icon, IconType, OverlayPopup, Spinner, TextInput, TooltipPlacement,
    checkbox_labeled, settings_file_picker, use_tooltip_anchor,
};
use crate::hooks::{
    add_collaborator, load_collaborators, load_owner_check, load_setting_files,
    remove_collaborator, use_dispatch, use_share_progress,
};
use crate::i18n::{t, tf};
use crate::state::{OwnerCheck, OwnerShare, ShareProgress, ShareReady};
use crate::theme::{self, colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::utils::count_label;

const DIALOG_WIDTH: f32 = 560.;
const CODE_FONT_SIZE: f32 = 52.;
const SHARED_CODE_FONT_SIZE: f32 = 28.;
const TEAM_LIST_MIN_HEIGHT: f32 = 260.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Copied {
    Code,
    Link,
}

#[derive(Clone, Copy)]
struct Picks {
    mode: State<ExportMode>,
    settings: State<bool>,
    files: FileChoice,
    picking: State<bool>,
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
        if !*self.settings.read() {
            return SettingsChoice::None;
        }
        match &*self.files.chosen.read() {
            None => SettingsChoice::All,
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
            SettingsChoice::None => self.settings.set(false),
            SettingsChoice::All | SettingsChoice::AllExcept(_) => {
                self.settings.set(true);
                chosen.set(None);
            }
            SettingsChoice::Only(paths) => {
                self.settings.set(true);
                chosen.set(Some(paths.clone()));
            }
        }
    }
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
            settings: use_state(|| false),
            files: FileChoice::use_new(),
            picking: use_state(|| false),
            team,
        };
        let copied = use_state(|| None::<Copied>);
        let saving = use_state(|| false);
        let file_mode = use_state(|| ExportMode::OneClient);
        let mut owner = use_state(|| OwnerCheck::Checking);
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

        use_hook(move || {
            let mut files = picks.files.files;
            spawn(async move { files.set(Some(load_setting_files(cluster_id).await)) });
            spawn(async move {
                let check = load_owner_check(cluster_id).await;
                if let OwnerCheck::Shared(shared) = &check {
                    picks.adopt(&shared.options);
                }
                owner.set(check);
            });
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

        let content = match share {
            None if *picks.picking.read() => picker_view(picks),
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
                copied,
                &dispatch,
                close.clone(),
            ),
        };

        let close_scrim = close.clone();
        OverlayPopup::new()
            .on_close(move |()| close_scrim.call(()))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(DIALOG_WIDTH))
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

fn title(text: String) -> Element {
    label()
        .text(text)
        .font_size(text_size(22.))
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_primary())
        .into_element()
}

fn header(text: String, close: EventHandler<()>) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(rect().width(Size::flex(1.)).child(title(text)))
        .child(
            Button::new()
                .ghost()
                .on_press(move |_| close.call(()))
                .child(Icon::new(IconType::XClose).size(16.)),
        )
        .into_element()
}

fn note(text: impl Into<String>, size: f32) -> Element {
    label()
        .text(text.into())
        .font_size(text_size(size))
        .color(colors::fg_secondary())
        .into_element()
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

fn copy_button(
    text: String,
    which: Copied,
    mut copied: State<Option<Copied>>,
    idle: &'static str,
    done: &'static str,
) -> Element {
    let shown = if *copied.read() == Some(which) {
        done
    } else {
        idle
    };
    Button::new()
        .secondary()
        .on_press(move |_| {
            if freya::text_edit::Clipboard::set(text.clone()).is_ok() {
                copied.set(Some(which));
            }
        })
        .text(shown)
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
                    .padding(Gaps::new(24., 0., 24., 0.))
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

    let mut code = rect().vertical().width(Size::flex(1.)).spacing(4.).child(
        label()
            .text(ready.code.clone())
            .font_size(text_size(SHARED_CODE_FONT_SIZE))
            .font_family(theme::mono_font())
            .font_weight(FontWeight::SEMI_BOLD)
            .color(colors::fg_strong()),
    );
    if let Some(expires) = &ready.expires {
        code = code.child(expiry(expires, 12.5));
    }

    Some(
        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(4.)
                    .padding(Gaps::new(16., 12., 16., 18.))
                    .corner_radius(rounded(12.))
                    .background(colors::component_bg())
                    .border(border_all_color(1., colors::component_border()))
                    .child(code)
                    .child(copy_icon(
                        ready.code.clone(),
                        Copied::Code,
                        copied,
                        IconType::Copy01,
                        t("oneclient.clusters.export.copy_code"),
                    ))
                    .child(copy_icon(
                        ready.link.clone(),
                        Copied::Link,
                        copied,
                        IconType::Link03,
                        t("oneclient.clusters.export.copy_link"),
                    )),
            )
            .child(status)
            .into_element(),
    )
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

fn settings_row(picks: Picks) -> Element {
    let mut picking = picks.picking;
    let included = *picks.settings.read();
    let files = picks.files.files.read();
    let summary = match &*files {
        _ if !included => None,
        None => Some(t("oneclient.clusters.export.settings.loading").to_string()),
        Some(list) if list.is_empty() => {
            Some(t("oneclient.clusters.export.settings.none").to_string())
        }
        Some(_) => {
            let (chosen, total) = picks.files.counts();
            Some(if chosen == total {
                count_label(
                    total,
                    t("oneclient.clusters.export.settings.all.one"),
                    t("oneclient.clusters.export.settings.all.other"),
                )
            } else if total == 1 {
                tf(
                    "oneclient.clusters.export.settings.some.one",
                    &[&chosen, &total],
                )
            } else {
                tf(
                    "oneclient.clusters.export.settings.some.other",
                    &[&chosen, &total],
                )
            })
        }
    };
    let has_files = files.as_ref().is_some_and(|list| !list.is_empty());
    drop(files);

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(checkbox_labeled(
            picks.settings,
            t("oneclient.clusters.export.settings.include"),
        ))
        .child(Hint {
            label: None,
            text: t("oneclient.clusters.export.settings.hint"),
        })
        .child(rect().width(Size::flex(1.)))
        .maybe_child(summary.map(|summary| note(summary, 12.5)))
        .maybe_child((included && has_files).then(|| {
            Button::new()
                .ghost()
                .small()
                .on_press(move |_| picking.set(true))
                .text(t("oneclient.clusters.export.settings.choose"))
                .into_element()
        }))
        .into_element()
}

fn share_header(name: &str, owner: &OwnerCheck, team: Team, close: EventHandler<()>) -> Element {
    let owns = matches!(owner, OwnerCheck::Shared(_) | OwnerCheck::Unknown(_));
    let (mut open, mut people, mut message) = (team.open, team.people, team.message);
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(4.)
        .child(
            rect()
                .width(Size::flex(1.))
                .child(title(tf("oneclient.clusters.export.share_title", &[&name]))),
        )
        .maybe_child(owns.then(|| {
            Button::new()
                .ghost()
                .icon()
                .tooltip(t("oneclient.clusters.export.team.title"))
                .on_press(move |_| {
                    people.set(None);
                    message.set(None);
                    open.set(true);
                })
                .child(Icon::new(IconType::Users01).size(16.))
                .into_element()
        }))
        .child(
            Button::new()
                .ghost()
                .on_press(move |_| close.call(()))
                .child(Icon::new(IconType::XClose).size(16.)),
        )
        .into_element()
}

#[allow(clippy::too_many_arguments)]
fn options_view(
    name: &str,
    cluster_id: i64,
    picks: Picks,
    owner: &OwnerCheck,
    copied: State<Option<Copied>>,
    mut saving: State<bool>,
    dispatch: &crate::hooks::Actions,
    close: EventHandler<()>,
) -> Vec<Element> {
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

    let mut elements = vec![share_header(name, owner, picks.team, close)];
    elements.push(owner_panel(owner, copied).unwrap_or_else(|| {
        Hint {
            label: Some(t("oneclient.clusters.export.how_sharing.label")),
            text: t("oneclient.clusters.export.how_sharing.text"),
        }
        .into_element()
    }));
    elements.push(settings_row(picks));

    let share_dispatch = dispatch.clone();
    let stop_dispatch = dispatch.clone();
    let right = vec![
        Button::new()
            .ghost()
            .on_press(move |_| saving.set(true))
            .text(t("oneclient.clusters.export.save_file_ellipsis"))
            .into_element(),
        Button::new()
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
    elements.push(footer(left, right));
    elements
}

fn save_view(
    name: &str,
    cluster_id: i64,
    picks: Picks,
    mut file_mode: State<ExportMode>,
    mut saving: State<bool>,
    dispatch: &crate::hooks::Actions,
    close: EventHandler<()>,
) -> Vec<Element> {
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
    vec![
        header(tf("oneclient.clusters.export.title", &[&name]), close),
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
        settings_row(picks),
        footer(
            Some(
                Button::new()
                    .ghost()
                    .on_press(move |_| saving.set(false))
                    .text(t("oneclient.common.back"))
                    .into_element(),
            ),
            vec![
                Button::new()
                    .on_press(move |_| {
                        let options = ExportOptions::new(*file_mode.peek())
                            .with_setting_choice(picks.choice());
                        export.export_cluster(cluster_id, options);
                        close_export.call(());
                    })
                    .text(t("oneclient.clusters.export.confirm"))
                    .into_element(),
            ],
        ),
    ]
}

fn team_view(team: Team, owner: &OwnerCheck) -> Vec<Element> {
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
    let mut elements = vec![
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
        elements.push(status_line(
            IconType::AlertTriangle,
            colors::danger(),
            &message,
        ));
    }
    elements.push(footer(
        None,
        vec![
            Button::new()
                .on_press(move |_| open.set(false))
                .text(t("oneclient.common.done"))
                .into_element(),
        ],
    ));
    elements
}

fn picker_view(picks: Picks) -> Vec<Element> {
    let mut picking = picks.picking;
    let mut elements = vec![
        title(t("oneclient.clusters.export.picker.title").to_string()),
        note(t("oneclient.clusters.export.picker.description"), 13.),
    ];
    elements.extend(settings_file_picker(picks.files));
    elements.push(footer(
        None,
        vec![
            Button::new()
                .on_press(move |_| picking.set(false))
                .text(t("oneclient.common.done"))
                .into_element(),
        ],
    ));
    elements
}

fn share_view(
    name: &str,
    cluster_id: i64,
    progress: ShareProgress,
    picks: Picks,
    copied: State<Option<Copied>>,
    dispatch: &crate::hooks::Actions,
    close: EventHandler<()>,
) -> Vec<Element> {
    let heading = title(tf("oneclient.clusters.export.share_title", &[&name]));
    match progress {
        ShareProgress::Working => vec![
            heading,
            rect()
                .vertical()
                .width(Size::fill())
                .padding(Gaps::new(28., 0., 28., 0.))
                .center()
                .spacing(12.)
                .child(Spinner::new(24.))
                .child(note(t("oneclient.clusters.export.uploading"), 13.))
                .into_element(),
            footer(
                None,
                vec![done_button(t("oneclient.clusters.export.hide"), close)],
            ),
        ],
        ShareProgress::Ready(ready) => ready_view(
            header(tf("oneclient.clusters.export.share_title", &[&name]), close),
            cluster_id,
            ready,
            false,
            copied,
            dispatch,
        ),
        ShareProgress::Stopping(ready) => ready_view(
            header(tf("oneclient.clusters.export.share_title", &[&name]), close),
            cluster_id,
            ready,
            true,
            copied,
            dispatch,
        ),
        ShareProgress::Stopped => vec![
            heading,
            status_line(
                IconType::CheckCircle,
                colors::success(),
                t("oneclient.clusters.export.stopped"),
            ),
            footer(None, vec![done_button(t("oneclient.common.done"), close)]),
        ],
        ShareProgress::Failed(reason) => {
            let retry = dispatch.clone();
            let close_cancel = close.clone();
            vec![
                heading,
                status_line(
                    IconType::AlertTriangle,
                    colors::danger(),
                    t("oneclient.clusters.export.failed"),
                ),
                note(reason, 13.),
                footer(
                    None,
                    vec![
                        Button::new()
                            .ghost()
                            .on_press(move |_| close_cancel.call(()))
                            .text(t("oneclient.common.close"))
                            .into_element(),
                        Button::new()
                            .on_press(move |_| retry.share_cluster(cluster_id, picks.options()))
                            .text(t("oneclient.common.try_again"))
                            .into_element(),
                    ],
                ),
            ]
        }
    }
}

fn ready_view(
    heading: Element,
    cluster_id: i64,
    ready: ShareReady,
    stopping: bool,
    copied: State<Option<Copied>>,
    dispatch: &crate::hooks::Actions,
) -> Vec<Element> {
    let stop = dispatch.clone();

    let mut code_box = rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .cross_align(Alignment::Center)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .padding(Gaps::new(28., 12., 28., 12.))
                .center()
                .corner_radius(rounded(12.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(
                    label()
                        .text(ready.code.clone())
                        .font_size(text_size(CODE_FONT_SIZE))
                        .font_family(theme::mono_font())
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_strong()),
                ),
        );
    if let Some(expires) = &ready.expires {
        code_box = code_box.child(expiry(expires, 13.));
    }

    let mut elements = vec![
        rect()
            .vertical()
            .spacing(6.)
            .child(heading)
            .child(Hint {
                label: Some(t("oneclient.clusters.export.how_import.label")),
                text: t("oneclient.clusters.export.how_import.text"),
            })
            .into_element(),
        code_box.into_element(),
    ];

    if ready.incomplete {
        elements.push(status_line(
            IconType::AlertTriangle,
            colors::danger(),
            t("oneclient.clusters.export.incomplete"),
        ));
    }

    let copies = vec![
        copy_button(
            ready.code.clone(),
            Copied::Code,
            copied,
            t("oneclient.clusters.export.copy_code"),
            t("oneclient.clusters.export.code_copied"),
        ),
        copy_button(
            ready.link.clone(),
            Copied::Link,
            copied,
            t("oneclient.clusters.export.copy_link"),
            t("oneclient.clusters.export.link_copied"),
        ),
    ];
    elements.push(footer(
        Some(
            Button::new()
                .ghost()
                .foreground(colors::danger())
                .enabled(!stopping)
                .on_press(move |_| stop.stop_sharing(cluster_id, ready.clone()))
                .text(if stopping {
                    t("oneclient.clusters.export.stopping")
                } else {
                    t("oneclient.clusters.export.stop")
                })
                .into_element(),
        ),
        copies,
    ));
    elements
}

#[derive(PartialEq)]
struct Hint {
    label: Option<&'static str>,
    text: &'static str,
}

impl Component for Hint {
    fn render(&self) -> impl IntoElement {
        let tooltip = use_tooltip_anchor(Some(self.text.into()), TooltipPlacement::Bottom);
        tooltip.attach(
            rect()
                .horizontal()
                .spacing(6.)
                .cross_align(Alignment::Center)
                .maybe_child(self.label.map(|label| note(label, 13.)))
                .child(
                    Icon::new(IconType::InfoCircle)
                        .size(14.)
                        .color(colors::fg_secondary()),
                ),
        )
    }
}

fn done_button(text: &'static str, close: EventHandler<()>) -> Element {
    Button::new()
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
