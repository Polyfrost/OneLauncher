use std::collections::HashMap;
use std::time::Duration;

use freya::prelude::*;
use freya::router::RouterContext;
use oneclient_core::servers::{self as core, LocalServer};

use super::{DASH, list_preview};
use crate::Route;
use crate::components::{
    Avatar, Button, Dropdown, Icon, IconType, ScrollArea, Segment, SegmentedControl, TextInput,
    memory_field, toggle_controlled, validate_number,
};
use crate::hooks::{
    ServerFiles, settled_or_loading, use_debounced_save, use_server, use_server_files,
    use_server_hub,
};
use crate::i18n::{t, tf};
use crate::servers;
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, centered_note};
use crate::view::app::settings::{resettable, settings_row, settings_row_danger};

const SIDEBAR_WIDTH: f32 = 190.;
const FIELD_WIDTH: f32 = 90.;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Section {
    Gameplay,
    World,
    Players,
    Network,
    Performance,
    Danger,
}

impl Section {
    const ALL: [Section; 6] = [
        Section::Gameplay,
        Section::World,
        Section::Players,
        Section::Network,
        Section::Performance,
        Section::Danger,
    ];

    fn label(self) -> &'static str {
        match self {
            Section::Gameplay => t("oneclient.servers.settings.section.gameplay"),
            Section::World => t("oneclient.servers.settings.section.world"),
            Section::Players => t("oneclient.servers.settings.section.players"),
            Section::Network => t("oneclient.servers.settings.section.network"),
            Section::Performance => t("oneclient.servers.settings.section.performance"),
            Section::Danger => t("oneclient.servers.settings.section.danger"),
        }
    }

    fn icon(self) -> IconType {
        match self {
            Section::Gameplay => IconType::Sliders04,
            Section::World => IconType::Globe01,
            Section::Players => IconType::Users01,
            Section::Network => IconType::Link03,
            Section::Performance => IconType::LineChartUp01,
            Section::Danger => IconType::AlertTriangle,
        }
    }
}

#[derive(Clone, PartialEq)]
enum Control {
    Toggle,
    Number,
    Text(f32),
    Choice(Options),
    Segments(Options),
}

type Options = Vec<(&'static str, &'static str)>;

fn gamemodes() -> Options {
    vec![
        (
            "survival",
            t("oneclient.servers.settings.gamemode.survival"),
        ),
        (
            "creative",
            t("oneclient.servers.settings.gamemode.creative"),
        ),
        (
            "adventure",
            t("oneclient.servers.settings.gamemode.adventure"),
        ),
        (
            "spectator",
            t("oneclient.servers.settings.gamemode.spectator"),
        ),
    ]
}

fn difficulties() -> Options {
    vec![
        (
            "peaceful",
            t("oneclient.servers.settings.difficulty.peaceful"),
        ),
        ("easy", t("oneclient.servers.settings.difficulty.easy")),
        ("normal", t("oneclient.servers.settings.difficulty.normal")),
        ("hard", t("oneclient.servers.settings.difficulty.hard")),
    ]
}

/// One server.properties key with its vanilla default
struct Prop {
    section: Section,
    key: &'static str,
    default: &'static str,
    control: Control,
    icon: IconType,
    title: &'static str,
    description: &'static str,
}

fn props() -> Vec<Prop> {
    let p = |section, key, default, control, icon, title, description| Prop {
        section,
        key,
        default,
        control,
        icon,
        title,
        description,
    };
    vec![
        p(
            Section::Gameplay,
            "gamemode",
            "survival",
            Control::Choice(gamemodes()),
            IconType::Rocket02,
            t("oneclient.servers.settings.gamemode.title"),
            t("oneclient.servers.settings.gamemode.description"),
        ),
        p(
            Section::Gameplay,
            "difficulty",
            "easy",
            Control::Segments(difficulties()),
            IconType::LineChartUp01,
            t("oneclient.servers.settings.difficulty.title"),
            t("oneclient.servers.settings.difficulty.description"),
        ),
        p(
            Section::Gameplay,
            "hardcore",
            "false",
            Control::Toggle,
            IconType::AlertCircle,
            t("oneclient.servers.settings.hardcore.title"),
            t("oneclient.servers.settings.hardcore.description"),
        ),
        p(
            Section::Gameplay,
            "pvp",
            "true",
            Control::Toggle,
            IconType::Users01,
            t("oneclient.servers.settings.pvp.title"),
            t("oneclient.servers.settings.pvp.description"),
        ),
        p(
            Section::Gameplay,
            "allow-flight",
            "false",
            Control::Toggle,
            IconType::Send01,
            t("oneclient.servers.settings.flight.title"),
            t("oneclient.servers.settings.flight.description"),
        ),
        p(
            Section::Gameplay,
            "enable-command-block",
            "false",
            Control::Toggle,
            IconType::Terminal,
            t("oneclient.servers.settings.command_blocks.title"),
            t("oneclient.servers.settings.command_blocks.description"),
        ),
        p(
            Section::World,
            "view-distance",
            "10",
            Control::Number,
            IconType::Eye,
            t("oneclient.servers.settings.view_distance.title"),
            t("oneclient.servers.settings.view_distance.description"),
        ),
        p(
            Section::World,
            "simulation-distance",
            "10",
            Control::Number,
            IconType::Settings04,
            t("oneclient.servers.settings.simulation_distance.title"),
            t("oneclient.servers.settings.simulation_distance.description"),
        ),
        p(
            Section::World,
            "spawn-protection",
            "16",
            Control::Number,
            IconType::Key01,
            t("oneclient.servers.settings.spawn_protection.title"),
            t("oneclient.servers.settings.spawn_protection.description"),
        ),
        p(
            Section::World,
            "level-seed",
            "",
            Control::Text(200.),
            IconType::DotsGrid,
            t("oneclient.servers.settings.seed.title"),
            t("oneclient.servers.settings.seed.description"),
        ),
        p(
            Section::Players,
            "max-players",
            "20",
            Control::Number,
            IconType::Users01,
            t("oneclient.servers.settings.max_players.title"),
            t("oneclient.servers.settings.max_players.description"),
        ),
        p(
            Section::Players,
            "online-mode",
            "true",
            Control::Toggle,
            IconType::Key01,
            t("oneclient.servers.settings.online_mode.title"),
            t("oneclient.servers.settings.online_mode.description"),
        ),
        p(
            Section::Players,
            "white-list",
            "false",
            Control::Toggle,
            IconType::CheckCircle,
            t("oneclient.servers.settings.whitelist.title"),
            t("oneclient.servers.settings.whitelist.description"),
        ),
        p(
            Section::Network,
            "server-port",
            "25565",
            Control::Number,
            IconType::Link03,
            t("oneclient.servers.settings.port.title"),
            t("oneclient.servers.settings.port.description"),
        ),
    ]
}

#[derive(PartialEq)]
pub struct ServerSettings {
    pub server_id: String,
}

impl Component for ServerSettings {
    fn render(&self) -> impl IntoElement {
        let server = use_server(&self.server_id);
        let files = settled_or_loading(&use_server_files(self.server_id.clone()));
        let search = use_state(String::new);
        let scroll = use_scroll_controller(ScrollConfig::default);
        let offsets = use_state(HashMap::<Section, f32>::new);
        let mut top = use_state(|| 0f32);

        let (Some(server), Some(files)) = (server, files) else {
            return centered_note(t("oneclient.servers.loading"));
        };

        let needle = search.read().trim().to_lowercase();
        let shows = |title: &str, description: &str| {
            needle.is_empty()
                || title.to_lowercase().contains(&needle)
                || description.to_lowercase().contains(&needle)
        };

        let mut by_section: HashMap<Section, Vec<Element>> = HashMap::new();
        for prop in props() {
            if !shows(prop.title, prop.description) {
                continue;
            }
            let value = files
                .props
                .get(prop.key)
                .unwrap_or_else(|| prop.default.to_string());
            by_section.entry(prop.section).or_default().push(
                PropRow {
                    server_id: self.server_id.clone(),
                    key: prop.key,
                    default: prop.default,
                    value: value.clone(),
                    control: prop.control,
                    icon: prop.icon,
                    title: prop.title,
                    description: prop.description,
                }
                .into_element(),
            );
        }

        let id = self.server_id.clone();
        let extra: [(Section, &'static str, &'static str, Element); 8] = [
            (
                Section::Players,
                t("oneclient.servers.settings.whitelisted.title"),
                "",
                whitelist_row(&id, &files, false),
            ),
            (
                Section::Network,
                t("oneclient.servers.settings.motd.title"),
                t("oneclient.servers.settings.motd.description"),
                MotdRow {
                    server: server.clone(),
                    files: files.clone(),
                }
                .into_element(),
            ),
            (
                Section::Performance,
                t("oneclient.servers.settings.memory.title"),
                t("oneclient.servers.settings.memory.description"),
                MemoryRow {
                    server: server.clone(),
                }
                .into_element(),
            ),
            (
                Section::Performance,
                t("oneclient.servers.settings.jvm.title"),
                t("oneclient.servers.settings.jvm.description"),
                JvmRow {
                    server: server.clone(),
                }
                .into_element(),
            ),
            (
                Section::Performance,
                t("oneclient.servers.settings.backups.title"),
                t("oneclient.servers.settings.backups.description"),
                backups_row(&server),
            ),
            (
                Section::Performance,
                t("oneclient.servers.settings.autostart.title"),
                t("oneclient.servers.settings.autostart.description"),
                autostart_row(&server),
            ),
            (
                Section::Danger,
                t("oneclient.servers.settings.reset.title"),
                t("oneclient.servers.settings.reset.description"),
                ConfirmRow {
                    server: server.clone(),
                    delete: false,
                }
                .into_element(),
            ),
            (
                Section::Danger,
                t("oneclient.servers.settings.delete.title"),
                t("oneclient.servers.settings.delete.description"),
                ConfirmRow {
                    server: server.clone(),
                    delete: true,
                }
                .into_element(),
            ),
        ];
        for (section, title, description, row) in extra {
            if shows(title, description) {
                by_section.entry(section).or_default().push(row);
            }
        }

        let sections: Vec<Element> = Section::ALL
            .into_iter()
            .filter_map(|section| {
                let rows = by_section.remove(&section)?;
                let mut offsets = offsets;
                Some(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(6.)
                        .on_sized(move |e: Event<SizedEventData>| {
                            let y = e.data().area.min_y();
                            if offsets.peek().get(&section) != Some(&y) {
                                offsets.write().insert(section, y);
                            }
                        })
                        .child(
                            rect().padding(Gaps::new(0., 0., 4., 2.)).child(
                                label()
                                    .text(section.label().to_uppercase())
                                    .font_size(text_size(11.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .letter_spacing(0.8)
                                    .color(colors::fg_secondary()),
                            ),
                        )
                        .children(rows)
                        .into_element(),
                )
            })
            .collect();

        let jump = move |section: Section| {
            let base = *top.peek();
            if let Some(y) = offsets.peek().get(&section) {
                let mut scroll = scroll;
                scroll.scroll_to_y(-((y - base).round() as i32));
            }
        };

        let props_path = server.dir().ok().map(|d| d.join("server.properties"));
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .spacing(32.)
            .child(sidebar(&server, &files, jump, props_path))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .spacing(12.)
                    .content(Content::Flex)
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .child(
                                label()
                                    .text(t("oneclient.servers.settings.title"))
                                    .width(Size::flex(1.0))
                                    .font_size(text_size(28.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                TextInput::new(search)
                                    .width(Size::px(240.))
                                    .placeholder(t("oneclient.servers.settings.search"))
                                    .leading(
                                        Icon::new(IconType::SearchMd)
                                            .size(16.)
                                            .color(colors::fg_secondary())
                                            .into_element(),
                                    ),
                            ),
                    )
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .scrollbar_gutter(true)
                            .scroll_controller(scroll)
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::fill())
                                    .spacing(18.)
                                    .padding(Gaps::new(0., 0., 24., 0.))
                                    .on_sized(move |e: Event<SizedEventData>| {
                                        let y = e.data().area.min_y();
                                        if *top.peek() != y {
                                            top.set(y);
                                        }
                                    })
                                    .maybe_child(sections.is_empty().then(|| {
                                        centered_note(t("oneclient.servers.settings.no_match"))
                                    }))
                                    .children(sections),
                            ),
                    ),
            )
            .into_element()
    }
}

fn sidebar(
    server: &LocalServer,
    files: &ServerFiles,
    jump: impl Fn(Section) + Clone + 'static,
    props_path: Option<std::path::PathBuf>,
) -> Element {
    let item = |icon: IconType, text: &'static str, on_press: EventHandler<()>| {
        SidebarItem {
            icon,
            text,
            on_press,
        }
        .into_element()
    };
    let group = |text: &'static str| {
        rect()
            .padding(Gaps::new(10., 0., 4., 8.))
            .child(
                label()
                    .text(text.to_uppercase())
                    .font_size(text_size(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .letter_spacing(0.8)
                    .color(colors::fg_secondary()),
            )
            .into_element()
    };
    let link = |section: Section| {
        let jump = jump.clone();
        item(
            section.icon(),
            section.label(),
            (move |()| jump(section)).into(),
        )
    };

    let java = server
        .java_major
        .map_or_else(|| DASH.to_string(), |j| j.to_string());
    let info = [
        format!("{} {} \u{b7} Java {java}", server.loader, server.mc_version),
        format!("servers/{}", server.id),
        tf(
            "oneclient.servers.settings.port_info",
            &[&files.props.get_or("server-port", "25565")],
        ),
    ];

    rect()
        .vertical()
        .width(Size::px(SIDEBAR_WIDTH))
        .spacing(2.)
        .child(group(t("oneclient.servers.settings.group.server")))
        .children(Section::ALL[..5].iter().map(|s| link(*s)))
        .child(group(t("oneclient.servers.settings.group.other")))
        .child(link(Section::Danger))
        .child(item(
            IconType::File02,
            "server.properties",
            (move |()| {
                if let Some(path) = &props_path {
                    crate::platform::open_path(&path.to_string_lossy());
                }
            })
            .into(),
        ))
        .child(
            rect()
                .vertical()
                .spacing(4.)
                .padding(Gaps::new(20., 0., 0., 8.))
                .children(info.into_iter().map(|line| {
                    label()
                        .text(line)
                        .font_size(text_size(11.))
                        .color(colors::fg_secondary())
                        .into_element()
                })),
        )
        .into_element()
}

#[derive(PartialEq)]
struct SidebarItem {
    icon: IconType,
    text: &'static str,
    on_press: EventHandler<()>,
}

impl Component for SidebarItem {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let on_press = self.on_press.clone();
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(33.))
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_symmetric(0., 10.))
            .corner_radius(rounded(7.))
            .background(if *hovering.read() {
                colors::ghost_overlay()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .on_press(move |_| on_press.call(()))
            .child(Icon::new(self.icon).size(16.).color(colors::fg_primary()))
            .child(
                label()
                    .text(self.text)
                    .font_size(text_size(14.))
                    .color(colors::fg_primary()),
            )
    }
}

fn row_key(parts: &[&str]) -> DiffKey {
    let mut hasher = std::hash::DefaultHasher::new();
    std::hash::Hash::hash(parts, &mut hasher);
    DiffKey::U64(std::hash::Hasher::finish(&hasher))
}

#[derive(PartialEq)]
struct PropRow {
    server_id: String,
    key: &'static str,
    default: &'static str,
    value: String,
    control: Control,
    icon: IconType,
    title: &'static str,
    description: &'static str,
}

impl Component for PropRow {
    fn render_key(&self) -> DiffKey {
        row_key(&[&self.server_id, self.key])
    }

    fn render(&self) -> impl IntoElement {
        let initial = self.value.clone();
        let mut value = use_state(move || initial);
        {
            let (id, key) = (self.server_id.clone(), self.key);
            use_debounced_save(
                value.read().clone(),
                Duration::from_millis(500),
                move |next| servers::set_property(id.clone(), key, next),
            );
        }

        let current = value.read().clone();
        let control: Element = match &self.control {
            Control::Toggle => {
                let on = current == "true";
                toggle_controlled(on, (move |()| value.set((!on).to_string())).into())
                    .into_element()
            }
            Control::Number => TextInput::new(value)
                .width(Size::px(FIELD_WIDTH))
                .on_validate(validate_number)
                .into_element(),
            Control::Text(width) => TextInput::new(value)
                .width(Size::px(*width))
                .font_family(crate::theme::mono_font())
                .into_element(),
            Control::Choice(options) => {
                let options = options.clone();
                let selected = options
                    .iter()
                    .find(|(v, _)| *v == current)
                    .map_or(current.clone(), |(_, label)| (*label).to_string());
                let labels = options
                    .iter()
                    .map(|(_, label)| (*label).to_string())
                    .collect();
                Dropdown::new(selected, labels)
                    .width(Size::px(160.))
                    .height(Size::px(34.))
                    .on_select(move |index: usize| {
                        if let Some((v, _)) = options.get(index) {
                            value.set((*v).to_string());
                        }
                    })
                    .into_element()
            }
            Control::Segments(options) => SegmentsControl {
                options: options.clone(),
                value,
            }
            .into_element(),
        };

        settings_row(
            self.icon,
            self.title,
            self.description,
            resettable(control, value, self.default.to_string()),
        )
    }
}

#[derive(PartialEq)]
struct SegmentsControl {
    options: Options,
    value: State<String>,
}

impl Component for SegmentsControl {
    fn render(&self) -> impl IntoElement {
        let options = self.options.clone();
        let mut value = self.value;
        let keys: Vec<&'static str> = options.iter().map(|(o, _)| *o).collect();
        let index_of = move |v: &str| keys.iter().position(|o| *o == v).unwrap_or(0);
        let mut index = use_state(|| index_of(&value.peek()));
        let from_value = index_of(&value.read());
        index.set_if_modified(from_value);
        let picked_options = options.clone();
        use_side_effect(move || {
            let picked = *index.read();
            if let Some((v, _)) = picked_options.get(picked)
                && *value.peek() != *v
            {
                value.set((*v).to_string());
            }
        });
        SegmentedControl::new(index).segments(
            options
                .iter()
                .enumerate()
                .map(|(i, (_, label))| Segment::new(i).label(*label)),
        )
    }
}

pub(super) fn whitelist_row(id: &str, files: &ServerFiles, op: bool) -> Element {
    let list = if op { &files.ops } else { &files.whitelist };
    WhitelistRow {
        server_id: id.to_string(),
        players: list
            .iter()
            .map(|p| (p.uuid.clone(), p.name.clone()))
            .collect(),
        ops: files.ops.iter().map(|p| p.name.to_lowercase()).collect(),
        op,
    }
    .into_element()
}

#[derive(PartialEq)]
struct WhitelistRow {
    server_id: String,
    players: Vec<(String, String)>,
    ops: Vec<String>,
    op: bool,
}

impl Component for WhitelistRow {
    fn render(&self) -> impl IntoElement {
        let mut name = use_state(String::new);
        let add_id = self.server_id.clone();
        let op_list = self.op;
        let mut add = move || {
            let entered = name.peek().trim().to_string();
            if !entered.is_empty() {
                servers::add_player(add_id.clone(), entered, op_list);
                name.set(String::new());
            }
        };
        let mut add_click = add.clone();

        let chips = self.players.iter().map(|(uuid, player)| {
            let op = !op_list && self.ops.contains(&player.to_lowercase());
            let (id, player_name) = (self.server_id.clone(), player.clone());
            rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .spacing(8.)
                .padding(Gaps::new(4., 6., 4., 6.))
                .corner_radius(rounded(8.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(Avatar::new(uuid.clone()).size(20.).radius(4.))
                .child(
                    label()
                        .text(player.clone())
                        .font_size(text_size(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .maybe_child(op.then(|| {
                    rect()
                        .padding(Gaps::new_symmetric(1., 6.))
                        .corner_radius(rounded(5.))
                        .background(colors::brand().with_a(50))
                        .child(
                            label()
                                .text("OP")
                                .font_size(text_size(10.))
                                .color(colors::brand()),
                        )
                        .into_element()
                }))
                .child(
                    Button::new()
                        .ghost()
                        .icon()
                        .small()
                        .alt(t("oneclient.servers.settings.whitelisted.remove"))
                        .on_press(move |_| {
                            servers::remove_player(id.clone(), player_name.clone(), op_list)
                        })
                        .child(Icon::new(IconType::XClose).size(12.)),
                )
                .into_element()
        });

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(12.)
            .padding(Gaps::new_symmetric(12., 16.))
            .corner_radius(rounded(12.))
            .background(colors::page_elevated())
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(16.)
                    .child(Icon::new(IconType::CheckCircle))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(2.)
                            .child(
                                label()
                                    .text(if op_list {
                                        t("oneclient.servers.settings.operators.title")
                                    } else {
                                        t("oneclient.servers.settings.whitelisted.title")
                                    })
                                    .font_size(text_size(16.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(if op_list {
                                        tf(
                                            "oneclient.servers.settings.operators.description",
                                            &[&self.players.len()],
                                        )
                                    } else {
                                        tf(
                                            "oneclient.servers.settings.whitelisted.description",
                                            &[&self.players.len()],
                                        )
                                    })
                                    .font_size(text_size(12.))
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(
                        TextInput::new(name)
                            .width(Size::px(150.))
                            .placeholder(t("oneclient.servers.settings.whitelisted.placeholder"))
                            .on_submit(move |_| add()),
                    )
                    .child(
                        Button::new()
                            .primary()
                            .on_press(move |_| add_click())
                            .child(Icon::new(IconType::Plus).size(14.))
                            .text(t("oneclient.servers.settings.whitelisted.add")),
                    ),
            )
            .maybe_child((!self.players.is_empty()).then(|| {
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::wrap_spacing(8.))
                    .spacing(8.)
                    .padding(Gaps::new(0., 0., 0., 40.))
                    .children(chips)
                    .into_element()
            }))
            .into_element()
    }
}

#[derive(PartialEq)]
struct MotdRow {
    server: LocalServer,
    files: ServerFiles,
}

impl Component for MotdRow {
    fn render_key(&self) -> DiffKey {
        row_key(&[&self.server.id, "MotdRow"])
    }

    fn render(&self) -> impl IntoElement {
        let initial = self.files.props.get_or("motd", "A Minecraft Server");
        let motd = use_state(move || initial);
        {
            let id = self.server.id.clone();
            use_debounced_save(
                motd.read().clone(),
                Duration::from_millis(600),
                move |next| servers::set_property(id.clone(), "motd", next),
            );
        }

        let live = use_server_hub().get(&self.server.id);
        let max = self.files.props.get_or("max-players", "20");
        let icon_id = self.server.id.clone();

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(12.)
            .padding(Gaps::new_symmetric(12., 16.))
            .corner_radius(rounded(12.))
            .background(colors::page_elevated())
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .spacing(16.)
                    .child(Icon::new(IconType::MessageTextSquare01))
                    .child(
                        rect()
                            .vertical()
                            .width(Size::flex(1.0))
                            .spacing(2.)
                            .child(
                                label()
                                    .text(t("oneclient.servers.settings.motd.title"))
                                    .font_size(text_size(16.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(t("oneclient.servers.settings.motd.description"))
                                    .font_size(text_size(12.))
                                    .color(colors::fg_secondary()),
                            ),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .on_press(move |_| {
                                let id = icon_id.clone();
                                spawn(async move {
                                    let Some(file) = rfd::AsyncFileDialog::new()
                                        .add_filter(
                                            t("oneclient.servers.settings.motd.icon_filter"),
                                            &["png", "jpg", "jpeg"],
                                        )
                                        .pick_file()
                                        .await
                                    else {
                                        return;
                                    };
                                    let source = file.path().to_path_buf();
                                    servers::run(
                                        t("oneclient.servers.settings.motd.icon_failed"),
                                        async move {
                                            core::set_icon(&core::server_dir(&id)?, source).await
                                        },
                                    );
                                });
                            })
                            .text(t("oneclient.servers.settings.motd.change_icon")),
                    ),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .spacing(10.)
                    .padding(Gaps::new(0., 0., 0., 40.))
                    .child(list_preview(
                        &self.server,
                        self.files.icon.clone(),
                        &motd.read(),
                        format!("{}/{max}", live.players.len()),
                    ))
                    .child(
                        TextInput::new(motd)
                            .width(Size::fill())
                            .font_family(crate::theme::mono_font())
                            .placeholder("A Minecraft Server"),
                    )
                    .child(super::motd::code_picker(motd)),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct MemoryRow {
    server: LocalServer,
}

impl Component for MemoryRow {
    fn render_key(&self) -> DiffKey {
        row_key(&[&self.server.id, "MemoryRow"])
    }

    fn render(&self) -> impl IntoElement {
        let initial = self.server.memory_mb.to_string();
        let memory = use_state(move || initial);
        let id = self.server.id.clone();
        use_debounced_save(
            memory.read().clone(),
            Duration::from_millis(600),
            move |next: String| {
                if let Ok(mb) = next.trim().parse::<u32>()
                    && mb >= 512
                {
                    servers::update(id.clone(), move |s| s.memory_mb = mb);
                }
            },
        );
        settings_row(
            IconType::Database01,
            t("oneclient.servers.settings.memory.title"),
            tf(
                "oneclient.servers.settings.memory.description",
                &[&core::DEFAULT_MEMORY_MB],
            ),
            resettable(
                memory_field(
                    memory,
                    t("oneclient.servers.settings.memory.default"),
                    core::DEFAULT_MEMORY_MB,
                ),
                memory,
                core::DEFAULT_MEMORY_MB.to_string(),
            ),
        )
    }
}

#[derive(PartialEq)]
struct JvmRow {
    server: LocalServer,
}

impl Component for JvmRow {
    fn render_key(&self) -> DiffKey {
        row_key(&[&self.server.id, "JvmRow"])
    }

    fn render(&self) -> impl IntoElement {
        let initial = self.server.jvm_args.clone();
        let args = use_state(move || initial);
        let id = self.server.id.clone();
        use_debounced_save(
            args.read().clone(),
            Duration::from_millis(600),
            move |next: String| {
                let next = next.trim().to_string();
                servers::update(id.clone(), move |s| s.jvm_args = next);
            },
        );
        let java = self
            .server
            .java_major
            .map_or_else(|| DASH.to_string(), |j| j.to_string());
        settings_row(
            IconType::CodeSnippet02,
            t("oneclient.servers.settings.jvm.title"),
            tf("oneclient.servers.settings.jvm.description", &[&java]),
            resettable(
                TextInput::new(args)
                    .width(Size::px(260.))
                    .font_family(crate::theme::mono_font())
                    .placeholder("-XX:+UseG1GC"),
                args,
                String::new(),
            ),
        )
    }
}

const BACKUP_HOURS: [u32; 6] = [0, 1, 3, 6, 12, 24];

fn backup_label(hours: u32) -> String {
    if hours == 0 {
        t("oneclient.servers.settings.off").to_string()
    } else {
        tf("oneclient.servers.settings.backups.every", &[&hours])
    }
}

fn backups_row(server: &LocalServer) -> Element {
    let id = server.id.clone();
    settings_row(
        IconType::ClockRewind,
        t("oneclient.servers.settings.backups.title"),
        t("oneclient.servers.settings.backups.description"),
        Dropdown::new(
            backup_label(server.backup_hours),
            BACKUP_HOURS.iter().map(|h| backup_label(*h)).collect(),
        )
        .width(Size::px(160.))
        .height(Size::px(34.))
        .on_select(move |index: usize| {
            let hours = BACKUP_HOURS[index.min(BACKUP_HOURS.len() - 1)];
            servers::update(id.clone(), move |s| s.backup_hours = hours);
        }),
    )
    .into_element()
}

fn autostart_row(server: &LocalServer) -> Element {
    let id = server.id.clone();
    let on = server.autostart;
    settings_row(
        IconType::Play,
        t("oneclient.servers.settings.autostart.title"),
        t("oneclient.servers.settings.autostart.description"),
        toggle_controlled(
            on,
            (move |()| servers::update(id.clone(), move |s| s.autostart = !on)).into(),
        ),
    )
    .into_element()
}

/// Asks for a second press before resetting the world or deleting the server
#[derive(PartialEq)]
pub(super) struct ConfirmRow {
    pub server: LocalServer,
    pub delete: bool,
}

impl Component for ConfirmRow {
    fn render(&self) -> impl IntoElement {
        let mut armed = use_state(|| false);
        let running = use_server_hub().status(&self.server.id).is_live();
        let (id, delete, keep) = (
            self.server.id.clone(),
            self.delete,
            self.server.backups_kept,
        );

        let button = Button::new()
            .small()
            .enabled(!running)
            .maybe(running, |b| {
                b.tooltip(t("oneclient.servers.settings.stop_first"))
            })
            .on_press(move |_| {
                if !*armed.peek() {
                    armed.set(true);
                    return;
                }
                armed.set(false);
                let id = id.clone();
                if delete {
                    let _ = RouterContext::get().replace(Route::Servers {});
                    servers::run(t("oneclient.servers.settings.delete.failed"), async move {
                        core::delete_server(&id).await
                    });
                } else {
                    servers::run(t("oneclient.servers.settings.reset.failed"), async move {
                        core::reset_world(&core::server_dir(&id)?, keep).await
                    });
                }
            });
        let button = match (delete, *armed.read()) {
            (_, true) => button
                .danger()
                .text(t("oneclient.servers.settings.confirm")),
            (true, false) => button
                .danger()
                .text(t("oneclient.servers.settings.delete.button")),
            (false, false) => button
                .secondary()
                .text(t("oneclient.servers.settings.reset.button")),
        };

        if delete {
            settings_row_danger(
                IconType::Trash01,
                t("oneclient.servers.settings.delete.title"),
                t("oneclient.servers.settings.delete.description"),
                button,
            )
            .into_element()
        } else {
            settings_row(
                IconType::RefreshCcw02,
                t("oneclient.servers.settings.reset.title"),
                t("oneclient.servers.settings.reset.description"),
                button,
            )
            .into_element()
        }
    }
}
