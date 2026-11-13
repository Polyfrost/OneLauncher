use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use bytes::Bytes;
use freya::prelude::*;
use oneclient_common::domain::ProviderId;
use oneclient_common::search::SearchQuery;
use oneclient_core::servers::{self as core, ServerMod};
use oneclient_core::settings::ViewLayout;

use crate::components::{
    Button, CARD_GRID_H, CARD_H, CardLayout, ContextMenu, FilterMenu, FilterOption, GRID_GAP,
    GRID_MIN_W, Icon, IconType, LazySection, PackageEntry, ScrollArea, Segment, SegmentedControl,
    TextInput, grid_card, icon_box, list_card,
};
use crate::hooks::{
    settled_or_loading, use_server, use_server_files, use_server_hub, use_view_state,
};
use crate::i18n::{t, tf};
use crate::servers;
use crate::theme::{colors, rounded, rounded_px, text_size};
use crate::ui::{ImageFallbackExt, centered_note, note};
use crate::view::app::cluster::confirm_dialog;
use crate::view::app::cluster::package_manager::{EnabledFilter, GRID_MAX_COLS, SortMode};
use crate::view::app::clusters::tidy_file_name;

const CONTROL_H: f32 = 34.;
const ROW_SPACING: f32 = 8.;
const ICON_SIZE: f32 = 44.;

fn entry(m: &ServerMod) -> PackageEntry {
    PackageEntry {
        package_id: m.file_name.clone(),
        bundle_name: None,
        provider: ProviderId::Local,
        github_hosted: false,
        github_url: None,
        name: m
            .name
            .clone()
            .unwrap_or_else(|| tidy_file_name(m.file_name.trim_end_matches(".disabled"))),
        file_name: m.file_name.clone(),
        author: m.authors.join(", "),
        version: m.version.clone(),
        description: m.description.clone().unwrap_or_default(),
        icon_url: None,
        size: m.size,
        categories: Vec::new(),
        enabled: m.enabled,
        installed: true,
        hash: None,
        manifest_default: false,
        hidden: false,
        opted_in: true,
        advanced: false,
        deleted: false,
        update_available: false,
        copy_status: None,
        copies: Vec::new(),
        seen_status: Default::default(),
        essential: None,
        bisect_role: None,
        locked: false,
        modpack: None,
        added_at: None,
        published_at: None,
    }
}

fn mod_icon(server_id: &str, item: &PackageEntry, icon: Option<&[u8]>) -> Element {
    match icon {
        Some(bytes) => ImageViewer::new((
            format!("server-mod:{server_id}:{}:{}", item.file_name, item.size),
            Bytes::copy_from_slice(bytes),
        ))
        .width(Size::px(ICON_SIZE))
        .height(Size::px(ICON_SIZE))
        .aspect_ratio(AspectRatio::Min)
        .corner_radius(rounded(8.))
        .fallback(icon_box(IconType::HelpCircle, ICON_SIZE))
        .into_element(),
        None => icon_box(IconType::HelpCircle, ICON_SIZE),
    }
}

/// The cluster mods list's look, minus what only applies to an instance:
/// bundles, the browser, updates and bisecting
#[derive(PartialEq)]
pub struct ServerMods {
    pub server_id: String,
}

impl Component for ServerMods {
    fn render(&self) -> impl IntoElement {
        let server = use_server(&self.server_id);
        let files = settled_or_loading(&use_server_files(self.server_id.clone()));
        let running = use_server_hub().status(&self.server_id).is_live();
        let search = use_state(String::new);
        let mut sort = use_state(|| SortMode::NameAsc);
        let mut show = use_state(|| EnabledFilter::All);
        let layout = use_view_state("server.mods").layout;
        let card_layout = CardLayout::from(*layout.read());
        let mut menu = use_state(|| None::<(f32, f32, PackageEntry)>);
        let mut deleting = use_state(|| None::<PackageEntry>);

        let (Some(server), Some(files)) = (server, files) else {
            return centered_note(t("oneclient.servers.loading"));
        };
        let Ok(dir) = server.dir() else {
            return centered_note(t("oneclient.servers.not_found"));
        };
        let mods_dir = dir.join("mods");
        let modded = server.loader.is_modded();

        let query = SearchQuery::new(&search.read());
        let current_sort = *sort.read();
        let current_show = *show.read();
        let mut entries: Vec<PackageEntry> = files
            .mods
            .iter()
            .map(entry)
            .filter(|e| current_show.keep(e))
            .filter(|e| {
                query.is_empty()
                    || query.matches(&e.name)
                    || query.matches(&e.file_name)
                    || query.matches(&e.author)
            })
            .collect();
        current_sort.sort(&mut entries);
        let rows = entries.into_iter().filter_map(|e| {
            let m = files.mods.iter().find(|m| m.file_name == e.file_name)?;
            Some((e, m))
        });

        let filter_menu = FilterMenu::new(
            current_sort != SortMode::NameAsc || current_show != EnabledFilter::All,
        )
        .section(
            t("oneclient.cluster.package_manager.filter.sort.title"),
            SortMode::ALL.map(|mode| {
                FilterOption::new(mode.label(), mode == current_sort, move |()| sort.set(mode))
            }),
        )
        .section(
            t("oneclient.cluster.package_manager.filter.show.title"),
            EnabledFilter::ALL.map(|filter| {
                FilterOption::new(filter.label(), filter == current_show, move |()| {
                    show.set(filter)
                })
            }),
        );

        let add_dir = mods_dir.clone();
        let toolbar = {
            let mut top = CornerRadius::new_all(0.);
            top.fill_top(rounded_px(12.));
            rect()
                .horizontal()
                .width(Size::fill())
                .content(Content::Flex)
                .cross_align(Alignment::Center)
                .spacing(8.)
                .padding(Gaps::new_symmetric(8., 12.))
                .corner_radius(top)
                .background(colors::page_elevated())
                .child(
                    label()
                        .text(tf("oneclient.servers.mods.count", &[&files.mods.len()]))
                        .width(Size::flex(1.0))
                        .font_size(text_size(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_secondary()),
                )
                .child(
                    TextInput::new(search)
                        .placeholder(t("oneclient.cluster.package_manager.search"))
                        .width(Size::px(180.))
                        .leading(
                            Icon::new(IconType::SearchMd)
                                .size(14.)
                                .color(colors::fg_secondary())
                                .into_element(),
                        ),
                )
                .child(filter_menu)
                .child(
                    SegmentedControl::new(layout)
                        .height(CONTROL_H)
                        .icon_size(15.)
                        .equal_width(CONTROL_H)
                        .segment(Segment::new(ViewLayout::List).icon(IconType::ParagraphWrap))
                        .segment(Segment::new(ViewLayout::Grid).icon(IconType::DotsGrid)),
                )
                .child(
                    crate::components::sized_open_folder_button(mods_dir.clone(), 16.)
                        .width(Size::px(CONTROL_H))
                        .height(Size::px(CONTROL_H)),
                )
                .child(
                    Button::new()
                        .primary()
                        .height(Size::px(CONTROL_H))
                        .font_size(text_size(12.))
                        .enabled(modded)
                        .on_press(move |_| add_files(add_dir.clone()))
                        .child(Icon::new(IconType::FilePlus02).size(15.))
                        .text(t("oneclient.cluster.package_manager.add_from_file")),
                )
        };

        let cards: Vec<ServerModCard> = rows
            .map(|(item, m)| ServerModCard {
                server_id: self.server_id.clone(),
                dir: dir.clone(),
                icon: m.icon.clone(),
                item,
                layout: card_layout,
                on_context: (move |(x, y, item)| menu.set(Some((x, y, item)))).into(),
            })
            .collect();

        let body: Element = if !modded {
            centered_note(t("oneclient.servers.mods.vanilla"))
        } else if files.mods.is_empty() {
            centered_note(t("oneclient.servers.mods.empty"))
        } else if cards.is_empty() {
            centered_note(t("oneclient.servers.mods.no_match"))
        } else {
            let (item_height, gap, min_width, max_cols) = match card_layout {
                CardLayout::List => (CARD_H, ROW_SPACING, 0., 1),
                CardLayout::Grid => (CARD_GRID_H, GRID_GAP, GRID_MIN_W, GRID_MAX_COLS),
            };
            let count = cards.len();
            ScrollArea::new()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .scrollbar_gutter(true)
                .lazy_sections(
                    vec![LazySection {
                        header: false,
                        count,
                    }],
                    item_height,
                    gap,
                    min_width,
                    max_cols,
                    0.,
                    move |i| cards[i].clone().into_element(),
                    |_| rect().into_element(),
                )
                .into_element()
        };

        let mut bottom = CornerRadius::new_all(0.);
        bottom.fill_bottom(rounded_px(12.));

        let context = menu.read().clone().map(|(x, y, item)| {
            let folder = mods_dir.clone();
            let target = item.clone();
            ContextMenu::new(x, y)
                .title(item.name.clone())
                .action(
                    IconType::Folder,
                    t("oneclient.components.package_row.view_in_folder"),
                    move |()| crate::platform::open_path(&folder.to_string_lossy()),
                )
                .separator()
                .danger_action(IconType::Trash01, t("oneclient.common.delete"), move |()| {
                    deleting.set(Some(target.clone()))
                })
                .on_close(move |_| menu.set(None))
                .into_element()
        });

        let delete_dialog = deleting.read().clone().map(|item| {
            let path = mods_dir.join(&item.file_name);
            confirm_dialog(
                tf("oneclient.servers.mods.delete.title", &[&item.name]),
                t("oneclient.servers.mods.delete.body").to_string(),
                move || deleting.set(None),
                move || {
                    let path = path.clone();
                    servers::run(t("oneclient.servers.mods.remove_failed"), async move {
                        tokio::fs::remove_file(path).await?;
                        Ok(())
                    });
                    deleting.set(None);
                },
            )
        });

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .spacing(10.)
            .maybe_child(running.then(|| {
                note(
                    t("oneclient.servers.mods.restart").to_string(),
                    colors::brand(),
                )
            }))
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .content(Content::Flex)
                    .child(toolbar)
                    .child(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .padding(Gaps::new(0., 12., 12., 12.))
                            .corner_radius(bottom)
                            .background(colors::page_elevated())
                            .overflow(Overflow::Clip)
                            .child(body),
                    ),
            )
            .maybe_child(context)
            .maybe_child(delete_dialog)
            .into_element()
    }
}

fn add_files(target: PathBuf) {
    spawn(async move {
        let Some(picked) = rfd::AsyncFileDialog::new()
            .set_title(t("oneclient.cluster.package_manager.import.picker_title"))
            .add_filter(t("oneclient.servers.mods.filter"), &["jar"])
            .pick_files()
            .await
        else {
            return;
        };
        let sources: Vec<PathBuf> = picked.iter().map(|f| f.path().to_path_buf()).collect();
        servers::run(t("oneclient.servers.mods.add_failed"), async move {
            tokio::fs::create_dir_all(&target).await?;
            for source in sources {
                if let Some(name) = source.file_name() {
                    tokio::fs::copy(&source, target.join(name)).await?;
                }
            }
            Ok(())
        });
    });
}

/// One mod as a list row or grid card, grid cards need their own hover state
#[derive(Clone, PartialEq)]
struct ServerModCard {
    server_id: String,
    dir: PathBuf,
    icon: Option<Vec<u8>>,
    item: PackageEntry,
    layout: CardLayout,
    on_context: EventHandler<(f32, f32, PackageEntry)>,
}

impl Component for ServerModCard {
    fn render(&self) -> impl IntoElement {
        let hovered = use_state(|| false);
        let item = self.item.clone();
        let (dir, file, enabled) = (self.dir.clone(), item.file_name.clone(), item.enabled);
        let on_toggle: EventHandler<()> = (move |()| {
            let (dir, file) = (dir.clone(), file.clone());
            servers::run(t("oneclient.servers.mods.toggle_failed"), async move {
                core::set_mod_enabled(&dir, &file, !enabled).await?;
                Ok(())
            });
        })
        .into();
        let on_context = self.on_context.clone();
        let for_menu = item.clone();
        let on_context: EventHandler<(f32, f32)> =
            (move |(x, y)| on_context.call((x, y, for_menu.clone()))).into();
        let icon = mod_icon(&self.server_id, &item, self.icon.as_deref());

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(match self.layout {
                CardLayout::List => {
                    list_card(&item, "mod", 0, icon, on_toggle, Some(on_context), None)
                }
                CardLayout::Grid => grid_card(
                    &item,
                    "mod",
                    0,
                    icon,
                    on_toggle,
                    false,
                    Some(on_context),
                    None,
                    hovered,
                ),
            })
    }

    fn render_key(&self) -> DiffKey {
        let mut hasher = DefaultHasher::new();
        self.item.package_id.hash(&mut hasher);
        DiffKey::U64(hasher.finish())
    }
}
