use super::super::install_target::{BrowseInstallPicker, InstallRequest};
use super::*;

use std::collections::HashMap;

use freya::router::RouterContext;
use oneclient_content::packages::types::ProjectSummary;
use oneclient_content::packages::{ContentType, ProviderId};
use oneclient_core::clusters::ModpackSource;

use crate::components::{Button, Icon, IconType};
use crate::hooks::{
    ALL_VERSIONS, ClusterAction, SearchOverride, VERSIONS_PAGE_SIZE, content_type_for_slug,
    use_cluster, use_dispatch, use_installs_snapshot, use_package_versions_when, version_list,
};
use crate::i18n::{t, tf};
use crate::routes::Route;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;
use crate::view::app::browser::{cluster_version, use_version_filter};

type InstalledMap = HashMap<(ProviderId, String), Installed>;

/// Height of the install control, matched by the installed pill that replaces it
const INSTALL_BUTTON_H: f32 = 28.;
/// Opaque rim behind the card icon so a see-through icon never shows the banner art through it
const ICON_PLATE: f32 = 2.;

/// Hover lifts the rim and fill a step so a card reads as pressable without an animation
fn card_surface(rect: Rect, focused: bool, hovered: bool) -> Rect {
    if colors::active_theme().liquid_glass {
        return crate::theme::glass::content(rect, colors::page_elevated(), 16.).maybe(
            focused || hovered,
            |el| {
                el.border(border_all_color(
                    1.,
                    if focused {
                        colors::brand()
                    } else {
                        colors::component_border_hover()
                    },
                ))
            },
        );
    }
    rect.corner_radius(rounded(8.))
        .background(if hovered {
            colors::component_bg_hover()
        } else {
            colors::component_bg()
        })
        .border(border_all_color(
            1.,
            if focused {
                colors::brand()
            } else if hovered {
                colors::component_border_hover()
            } else {
                colors::component_border()
            },
        ))
}

#[derive(Clone, PartialEq)]
struct CardInstalled {
    source: InstallSource,
    enable: Option<ClusterAction>,
}

fn installed_for(
    installed: &InstalledMap,
    item: &ProjectSummary,
    cluster_id: i64,
) -> Option<CardInstalled> {
    installed
        .get(&(item.provider, item.id.clone()))
        .map(|installed| CardInstalled {
            source: installed.source,
            enable: installed
                .disabled_bundled()
                .and_then(|version| version.enable_action(cluster_id)),
        })
}

pub(super) fn grid_row(
    row: Vec<ProjectSummary>,
    cluster_id: i64,
    package_type: &str,
    installed: &InstalledMap,
    cols: usize,
    add_to_cluster: bool,
    search_override: Option<SearchOverride>,
) -> impl IntoElement {
    let package_type = package_type.to_string();
    let fill = cols.saturating_sub(row.len());
    let installed: Vec<Option<CardInstalled>> = row
        .iter()
        .map(|item| installed_for(installed, item, cluster_id))
        .collect();

    rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::px(CARD_H))
        .spacing(GRID_SPACING)
        .content(Content::Flex)
        .children(
            row.into_iter()
                .zip(installed)
                .map(move |(item, installed)| {
                    PackageCard::new(
                        item,
                        cluster_id,
                        package_type.clone(),
                        installed,
                        add_to_cluster,
                        search_override.clone(),
                    )
                    .into_element()
                }),
        )
        .maybe(fill > 0, move |mut el| {
            for _ in 0..fill {
                el = el.child(rect().width(Size::flex(1.0)));
            }
            el
        })
        .into_element()
}

fn open_package(
    cluster_id: i64,
    package_type: &str,
    provider: ProviderId,
    id: &str,
    add_to_cluster: bool,
) {
    let router = RouterContext::get();
    let in_instance = matches!(
        router.current::<Route>(),
        Route::Browser {
            in_instance: true,
            ..
        }
    );
    let _ = router.push(Route::BrowserPackage {
        cluster_id,
        package_type: package_type.to_string(),
        package_id: encode_package_id(provider, id),
        add_to_cluster,
        in_instance,
    });
}

#[derive(PartialEq)]
struct PackageCard {
    item: ProjectSummary,
    cluster_id: i64,
    package_type: String,
    installed: Option<CardInstalled>,
    add_to_cluster: bool,
    search_override: Option<SearchOverride>,
}

impl PackageCard {
    fn new(
        item: ProjectSummary,
        cluster_id: i64,
        package_type: String,
        installed: Option<CardInstalled>,
        add_to_cluster: bool,
        search_override: Option<SearchOverride>,
    ) -> Self {
        Self {
            item,
            cluster_id,
            package_type,
            installed,
            add_to_cluster,
            search_override,
        }
    }
}

impl Component for PackageCard {
    fn render(&self) -> impl IntoElement {
        let id = self.item.id.clone();
        let provider = self.item.provider;
        let package_type = self.package_type.clone();
        let icon_url = self.item.icon_url.clone();
        let cluster_id = self.cluster_id;
        let add_to_cluster = self.add_to_cluster;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus() == Focus::Keyboard;
        let mut hovering = use_state(|| false);
        let hovered = *hovering.read();

        let header = rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(BANNER_H))
            .overflow(Overflow::Clip)
            .cross_align(Alignment::Center)
            .content(Content::Flex)
            .spacing(10.)
            .padding(Gaps::new_symmetric(0., 14.))
            .background(colors::ghost_overlay())
            .child(
                rect()
                    .position(Position::new_absolute().top(BANNER_H - 1.).left(0.))
                    .layer(Layer::Relative(8))
                    .width(Size::fill())
                    .height(Size::px(1.))
                    .background(colors::component_border()),
            )
            .child(
                // Opaque rim so a see-through icon never shows the artwork through it
                rect()
                    .layer(Layer::Relative(7))
                    .padding(Gaps::new_all(ICON_PLATE))
                    .corner_radius(rounded(6. + ICON_PLATE))
                    .background(if hovered && !colors::active_theme().liquid_glass {
                        colors::component_bg_hover()
                    } else {
                        colors::component_bg()
                    })
                    .child(Thumbnail::new(icon_url, CARD_ICON).radius(6.)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .layer(Layer::Relative(7))
                    .spacing(4.)
                    .child(
                        label()
                            .text(self.item.name.clone())
                            .font_size(text_size(14.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .width(Size::fill())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .spacing(5.)
                            .child(Icon::new(self.item.provider).size(11.))
                            .child(
                                label()
                                    .text(self.item.author.clone())
                                    .font_size(text_size(11.))
                                    .width(Size::flex(1.0))
                                    .max_lines(1)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .color(colors::fg_secondary()),
                            )
                            .child(downloads_row(self.item.downloads)),
                    ),
            );

        card_surface(rect(), focused, hovered)
            .vertical()
            .width(Size::flex(1.0))
            .height(Size::px(CARD_H))
            .overflow(Overflow::Clip)
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| {
                open_package(cluster_id, &package_type, provider, &id, add_to_cluster)
            })
            // No installed badge, the control at the foot of the card already states it
            .child(header)
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .padding(Gaps::new(10., 14., 12., 14.))
                    .spacing(8.)
                    .content(Content::Flex)
                    .child(
                        // Takes what the install control leaves and clips, so a tall font or a
                        // narrow card can never run the summary into the button
                        rect()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .overflow(Overflow::Clip)
                            .child(
                                label()
                                    .text(self.item.summary.clone())
                                    .font_size(text_size(11.))
                                    .line_height(1.45)
                                    .max_lines(2)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .width(Size::fill())
                                    .color(colors::fg_primary().with_a(190)),
                            ),
                    )
                    .child(InstallButton::new(
                        &self.item,
                        self.cluster_id,
                        &self.package_type,
                        self.installed.clone(),
                        self.add_to_cluster,
                        self.search_override.clone(),
                    )),
            )
    }
}

pub(super) fn list_row(
    item: ProjectSummary,
    cluster_id: i64,
    package_type: &str,
    installed: &InstalledMap,
    add_to_cluster: bool,
    search_override: Option<SearchOverride>,
    compact: bool,
) -> impl IntoElement {
    ListRow {
        installed: installed_for(installed, &item, cluster_id),
        item,
        cluster_id,
        package_type: package_type.to_string(),
        add_to_cluster,
        search_override,
        compact,
    }
}

#[derive(PartialEq)]
struct ListRow {
    item: ProjectSummary,
    cluster_id: i64,
    package_type: String,
    installed: Option<CardInstalled>,
    add_to_cluster: bool,
    search_override: Option<SearchOverride>,
    /// Too narrow for the author beside the name and a downloads column of its own
    compact: bool,
}

impl Component for ListRow {
    fn render(&self) -> impl IntoElement {
        let item = self.item.clone();
        let cluster_id = self.cluster_id;
        let id = item.id.clone();
        let provider = item.provider;
        let package_type = self.package_type.clone();
        let add_to_cluster = self.add_to_cluster;
        let compact = self.compact;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let focused = focus() == Focus::Keyboard;
        let mut hovering = use_state(|| false);
        let hovered = *hovering.read();

        let name = label()
            .text(item.name.clone())
            .font_size(text_size(15.))
            .font_weight(FontWeight::SEMI_BOLD)
            .max_lines(1)
            .text_overflow(TextOverflow::Ellipsis)
            .color(colors::fg_primary());
        let meta = |text: String| {
            label()
                .text(text)
                .font_size(text_size(11.))
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .color(colors::fg_primary().with_a(140))
        };
        let summary = label()
            .text(item.summary.clone())
            .font_size(text_size(11.))
            .max_lines(if compact { 1 } else { 2 })
            .text_overflow(TextOverflow::Ellipsis)
            .width(Size::fill())
            .color(colors::fg_primary().with_a(184));

        // Clipped so nothing in here can paint over the downloads count or the install control
        let text = rect()
            .vertical()
            .width(Size::flex(1.0))
            .overflow(Overflow::Clip)
            .spacing(3.);
        let text = if compact {
            text.child(name.width(Size::fill()))
                .child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .spacing(5.)
                        .child(Icon::new(item.provider).size(11.))
                        .child(
                            meta(tf(
                                "oneclient.browser.index.card.author_downloads",
                                &[&item.author, &abbreviate_number(item.downloads)],
                            ))
                            .width(Size::flex(1.0)),
                        ),
                )
                .child(summary)
        } else {
            text.child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .cross_align(Alignment::Center)
                    .spacing(6.)
                    .child(name)
                    .child(meta(tf(
                        "oneclient.browser.index.card.author",
                        &[&item.author],
                    )))
                    .child(Icon::new(item.provider).size(12.)),
            )
            .child(summary)
        };

        card_surface(rect(), focused, hovered)
            .horizontal()
            .width(Size::fill())
            .height(Size::px(LIST_ROW_H))
            .on_pointer_enter(move |_| hovering.set(true))
            .on_pointer_leave(move |_| hovering.set(false))
            .cross_align(Alignment::Center)
            .spacing(12.)
            .padding(Gaps::new_symmetric(0., 14.))
            .overflow(Overflow::Clip)
            .content(Content::Flex)
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| {
                open_package(cluster_id, &package_type, provider, &id, add_to_cluster)
            })
            .child(Thumbnail::new(item.icon_url.clone(), 48.).radius(6.))
            .child(text)
            .maybe_child((!compact).then(|| downloads_row(item.downloads)))
            .child(InstallButton::new(
                &self.item,
                self.cluster_id,
                &self.package_type,
                self.installed.clone(),
                self.add_to_cluster,
                self.search_override.clone(),
            ))
    }
}

pub(super) fn instance_row(
    item: ProjectSummary,
    cluster_id: i64,
    package_type: &str,
    installed: &InstalledMap,
    add_to_cluster: bool,
    search_override: Option<SearchOverride>,
    compact: bool,
) -> impl IntoElement {
    InstanceRow {
        installed: installed_for(installed, &item, cluster_id),
        item,
        cluster_id,
        package_type: package_type.to_string(),
        add_to_cluster,
        search_override,
        compact,
    }
}

/// A result inside the instance pop-up, on the same row surface as the instance's content pages
#[derive(PartialEq)]
struct InstanceRow {
    item: ProjectSummary,
    cluster_id: i64,
    package_type: String,
    installed: Option<CardInstalled>,
    add_to_cluster: bool,
    search_override: Option<SearchOverride>,
    /// Too narrow for the downloads count beside the install control
    compact: bool,
}

impl Component for InstanceRow {
    fn render(&self) -> impl IntoElement {
        let item = &self.item;
        let cluster_id = self.cluster_id;
        let id = item.id.clone();
        let provider = item.provider;
        let package_type = self.package_type.clone();
        let add_to_cluster = self.add_to_cluster;

        let a11y_id = use_a11y();
        let focus = use_focus(a11y_id);
        let hovered = use_state(|| false);
        let author = (!item.author.is_empty())
            .then(|| tf("oneclient.browser.index.card.author", &[&item.author]));
        let line = crate::components::joined_line(
            author
                .iter()
                .map(String::as_str)
                .chain([item.summary.as_str()]),
        );

        crate::components::row_surface(hovered)
            .maybe(focus() == Focus::Keyboard, |el| {
                el.border(border_all_color(1., colors::brand()).alignment(BorderAlignment::Inner))
            })
            .a11y_id(a11y_id)
            .a11y_focusable(true)
            .a11y_role(AccessibilityRole::Button)
            .a11y_alt(item.name.clone())
            .cursor(CursorIcon::Pointer)
            .on_press(move |_| {
                open_package(cluster_id, &package_type, provider, &id, add_to_cluster)
            })
            .child(Thumbnail::new(item.icon_url.clone(), crate::components::ROW_ICON).radius(8.))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(4.)
                    .child(crate::components::row_title(
                        item.name.clone(),
                        colors::fg_primary(),
                    ))
                    .maybe(!line.is_empty(), |el| {
                        el.child(crate::components::row_line(line, colors::fg_secondary()))
                    }),
            )
            .maybe_child((!self.compact).then(|| {
                rect()
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .spacing(4.)
                    .child(
                        Icon::new(IconType::Download01)
                            .size(12.)
                            .color(colors::fg_secondary()),
                    )
                    .child(
                        label()
                            .text(abbreviate_number(item.downloads))
                            .font_size(text_size(12.))
                            .color(colors::fg_secondary()),
                    )
            }))
            .child(InstallButton::new(
                &self.item,
                self.cluster_id,
                &self.package_type,
                self.installed.clone(),
                self.add_to_cluster,
                self.search_override.clone(),
            ))
    }
}

#[derive(PartialEq)]
struct InstallButton {
    provider: ProviderId,
    project_id: String,
    project_name: String,
    cluster_id: i64,
    content_type: ContentType,
    package_type: String,
    search_override: Option<SearchOverride>,
    /// Cluster already has this one, so the control becomes a static pill
    installed: Option<CardInstalled>,
    add_to_cluster: bool,
}

impl InstallButton {
    fn new(
        item: &ProjectSummary,
        cluster_id: i64,
        package_type: &str,
        installed: Option<CardInstalled>,
        add_to_cluster: bool,
        search_override: Option<SearchOverride>,
    ) -> Self {
        let content_type = content_type_for_slug(package_type);
        Self {
            provider: item.provider,
            project_id: item.id.clone(),
            project_name: item.name.clone(),
            cluster_id,
            content_type,
            package_type: package_type.to_string(),
            search_override,
            installed,
            add_to_cluster: add_to_cluster && content_type == ContentType::Modpack,
        }
    }
}

impl Component for InstallButton {
    fn render(&self) -> impl IntoElement {
        let cluster_id = self.cluster_id;
        let provider = self.provider;
        let project_id = self.project_id.clone();

        let dispatch = use_dispatch();
        let cluster = use_cluster(cluster_id);

        // The same narrowing the package page does so both agree on what "latest" installs
        let is_modpack = self.content_type == ContentType::Modpack;
        let choose_target = cluster_id == 0 && !is_modpack;
        let (game_version, loader, incompatible_mode) =
            use_version_filter(cluster_id, self.content_type, self.search_override.clone());

        let versions = version_list(&use_package_versions_when(
            self.installed.is_none() && !choose_target,
            provider,
            project_id.clone(),
            game_version,
            loader,
            0,
            if is_modpack {
                ALL_VERSIONS
            } else {
                VERSIONS_PAGE_SIZE
            },
        ));
        let add_to_cluster = self.add_to_cluster;
        let latest = if add_to_cluster {
            cluster.as_ref().and_then(|c| cluster_version(&versions, c))
        } else {
            preferred_version(&versions, self.content_type)
        }
        .map(|v| v.version_id.clone());
        let is_datapack = self.content_type == ContentType::DataPack;
        let mut world_prompt = use_state(|| None::<(String, crate::install::IncompatibleMode)>);
        let mut modpack_prompt = use_state(|| false);
        let mut choices = use_state(Vec::new);
        let mut target_prompt = use_state(|| false);

        // Nothing to start twice while an install is running or before versions arrive
        let (installing, waiting) =
            use_installs_snapshot().package_busy(is_modpack, cluster_id, provider, &project_id);

        if let Some(installed) = &self.installed {
            if let Some(action) = installed.enable.clone() {
                return EnableButton {
                    action,
                    variant: EnableVariant::Card {
                        height: INSTALL_BUTTON_H,
                    },
                }
                .into_element();
            }
            let installed = installed.source;
            let color = installed.color();
            return rect()
                .horizontal()
                .cross_align(Alignment::Center)
                .height(Size::px(INSTALL_BUTTON_H))
                .spacing(6.)
                .padding(Gaps::new_symmetric(0., 10.))
                .corner_radius(control_radius(INSTALL_BUTTON_H, 6.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(Icon::new(IconType::CheckCircle).size(12.).color(color))
                .child(
                    label()
                        .text(installed.label())
                        .font_size(text_size(11.))
                        .font_weight(FontWeight::NORMAL)
                        .max_lines(1)
                        .color(colors::fg_secondary()),
                )
                .into_element();
        }

        let incompatible = add_to_cluster && latest.is_none() && !versions.is_empty();

        rect()
            // The card behind is one big press target stop here so the package page doesn't open on top of the install
            .on_press(|e: Event<PressEventData>| e.stop_propagation())
            .child(
                Button::new()
                    .primary()
                    .small()
                    .height(Size::px(INSTALL_BUTTON_H))
                    .padding(Gaps::new_symmetric(0., 11.))
                    .corner_radius(control_radius(INSTALL_BUTTON_H, 6.))
                    .enabled((choose_target || latest.is_some()) && !installing && !waiting)
                    .on_press({
                        let project_id = project_id.clone();
                        move |_| {
                            if choose_target {
                                target_prompt.set(true);
                                return;
                            }
                            let Some(version_id) = latest.clone() else {
                                return;
                            };
                            if add_to_cluster {
                                dispatch.import_modpack(
                                    cluster_id,
                                    ModpackSource::Provider {
                                        provider,
                                        project_id: project_id.clone(),
                                        version_id,
                                    },
                                );
                            } else if is_datapack {
                                world_prompt
                                    .set(Some((version_id, crate::install::IncompatibleMode::Ask)));
                            } else if is_modpack {
                                let picks = minecraft_choices(&versions);
                                if picks.len() > 1 {
                                    choices.set(picks);
                                    modpack_prompt.set(true);
                                } else {
                                    dispatch.install_modpack(ModpackSource::Provider {
                                        provider,
                                        project_id: project_id.clone(),
                                        version_id,
                                    });
                                }
                            } else {
                                dispatch.install_package(
                                    cluster_id,
                                    provider,
                                    project_id.clone(),
                                    version_id,
                                    None,
                                    incompatible_mode,
                                );
                            }
                        }
                    })
                    .child(
                        Icon::new(if installing {
                            IconType::Loading02
                        } else {
                            IconType::Plus
                        })
                        .size(12.)
                        .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(match (installing, add_to_cluster) {
                                (true, true) => t("oneclient.browser.index.card.adding"),
                                (true, false) => t("oneclient.browser.index.card.installing"),
                                (false, true) if incompatible => {
                                    t("oneclient.browser.index.card.incompatible")
                                }
                                (false, true) => t("oneclient.common.add"),
                                (false, false) => t("oneclient.common.install"),
                            })
                            .font_size(text_size(11.))
                            .font_weight(FontWeight::SEMI_BOLD)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    ),
            )
            .maybe_child(target_prompt.read().then(|| BrowseInstallPicker {
                provider,
                content_type: self.content_type,
                package_type: self.package_type.clone(),
                install: InstallRequest {
                    project_id: project_id.clone(),
                    project_name: self.project_name.clone(),
                    version: None,
                },
                on_close: (move |()| target_prompt.set(false)).into(),
            }))
            .maybe_child(world_prompt.read().is_some().then_some(WorldInstallPrompt {
                cluster_id,
                provider,
                project_id: project_id.clone(),
                pending: world_prompt,
            }))
            .maybe_child(modpack_prompt.read().then(|| ModpackVersionPrompt {
                provider,
                project_id,
                choices: choices.read().clone(),
                open: modpack_prompt,
            }))
            .into_element()
    }
}

fn downloads_row(downloads: u64) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .child(
            Icon::new(IconType::Download01)
                .size(12.)
                .color(colors::fg_primary().with_a(140)),
        )
        .child(
            label()
                .text(abbreviate_number(downloads))
                .font_size(text_size(11.))
                .color(colors::fg_primary().with_a(140)),
        )
}

pub(super) fn empty_state() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .child(
            label()
                .text(t("oneclient.browser.index.no_results"))
                .font_size(text_size(14.))
                .color(colors::fg_secondary()),
        )
}
