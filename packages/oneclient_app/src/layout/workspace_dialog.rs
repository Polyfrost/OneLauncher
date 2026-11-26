use freya::{animation::*, prelude::*, router::*};

use crate::Route;
use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::i18n::t;
use crate::theme::{NAVBAR_HEIGHT_PX, colors, glass, rounded, rounded_px, text_size};
use crate::ui::border_all_color;

const NAVBAR_GAP_PX: f32 = 8.;
const WINDOW_GUTTER_PX: f32 = 24.;
const COMPACT_GUTTER_PX: f32 = 12.;
const HEADER_INSET_PX: f32 = 20.;
const ENTER_SHIFT_PX: f32 = 32.;

#[derive(Clone, Copy)]
pub(super) struct WorkspaceLayout {
    pub compact: bool,
    pub inset: f32,
    pub row_height: f32,
    pub row_gap: f32,
}

impl WorkspaceLayout {
    pub fn current() -> Self {
        let window = *Platform::get().current_window().root_size.read();
        Self::for_size(window.width, window.height)
    }

    pub fn for_size(width: f32, height: f32) -> Self {
        let compact = height < 760.;
        Self {
            compact,
            inset: if compact || width < 1000. { 12. } else { 16. },
            row_height: if compact { 28. } else { 34. },
            row_gap: if compact { 2. } else { 3. },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WorkspaceDialog {
    Instance,
    Settings,
    Statistics,
}

impl WorkspaceDialog {
    pub(super) fn for_route(route: &Route) -> Option<Self> {
        Some(match route {
            Route::ClusterOverview { .. }
            | Route::ClusterLogs { .. }
            | Route::ProcessLogs { .. }
            | Route::ClusterScreenshots { .. }
            | Route::ClusterMods { .. }
            | Route::ClusterShaders { .. }
            | Route::ClusterTextures { .. }
            | Route::ClusterWorlds { .. }
            | Route::ClusterDataPacks { .. }
            | Route::ClusterSettings { .. }
            | Route::Browser {
                in_instance: true, ..
            }
            | Route::BrowserPackage {
                in_instance: true, ..
            } => Self::Instance,
            Route::SettingsAppearance {}
            | Route::SettingsThemeEditor { .. }
            | Route::SettingsMinecraft {}
            | Route::SettingsAccounts {}
            | Route::SettingsLauncher {}
            | Route::SettingsJava {}
            | Route::SettingsStorage {}
            | Route::SettingsApis {}
            | Route::SettingsLanguage {}
            | Route::SettingsDeveloper {}
            | Route::SettingsChangelog {} => Self::Settings,
            Route::Stats {} => Self::Statistics,
            _ => return None,
        })
    }

    fn title(self) -> &'static str {
        match self {
            Self::Instance => t("oneclient.layout.workspace.instance"),
            Self::Settings => t("oneclient.components.control_center.settings"),
            Self::Statistics => t("oneclient.stats.title"),
        }
    }
}

#[derive(Clone, Copy)]
struct AppPage(State<Route>);

pub(super) fn provide_app_page(route: &Route) -> Route {
    let mut page = use_state(|| match WorkspaceDialog::for_route(route) {
        Some(WorkspaceDialog::Instance) => Route::Clusters {},
        Some(_) => Route::Home {},
        None => route.clone(),
    });
    use_hook(move || provide_context(AppPage(page)));
    if WorkspaceDialog::for_route(route).is_none() && &*page.peek() != route {
        page.set(route.clone());
    }
    page.read().clone()
}

pub(crate) fn use_app_page() -> Route {
    consume_context::<AppPage>().0.read().clone()
}

/// Render a specific route at this outlet level so a dialog can leave its originating page mounted.
#[derive(PartialEq)]
pub(super) struct RoutedPage(pub Route);

impl Component for RoutedPage {
    fn render(&self) -> impl IntoElement {
        self.0.render(use_outlet_context::<Route>().level())
    }
}

#[derive(PartialEq)]
pub(super) struct WorkspacePopup {
    pub(super) kind: WorkspaceDialog,
    pub(super) route: Route,
    pub(super) origin: Route,
}

fn dismiss(origin: &Route) {
    let router = RouterContext::get();
    let mut steps = 0;
    while WorkspaceDialog::for_route(&router.current::<Route>()).is_some() && router.can_go_back() {
        router.go_back();
        steps += 1;
    }
    if router.current::<Route>() != *origin {
        // Some entry points replace a route (for example a system shortcut).
        // Keep their existing history and restore the visible page.
        for _ in 0..steps {
            router.go_forward();
        }
        let _ = router.replace(origin.clone());
    }
}

impl Component for WorkspacePopup {
    fn render(&self) -> impl IntoElement {
        let browsing_in_instance = matches!(
            &self.route,
            Route::Browser {
                in_instance: true,
                ..
            } | Route::BrowserPackage {
                in_instance: true,
                ..
            }
        );
        let show_back = browsing_in_instance && RouterContext::get().can_go_back();
        let window = *Platform::get().current_window().root_size.read();
        let layout = WorkspaceLayout::for_size(window.width, window.height);
        let header_height = if layout.compact && self.kind != WorkspaceDialog::Statistics {
            40.
        } else {
            48.
        };
        let compact = window.width < 1000. || window.height < 720.;
        let gutter = if compact {
            COMPACT_GUTTER_PX
        } else {
            WINDOW_GUTTER_PX
        };
        let panel_width = if compact {
            Size::fill()
        } else {
            Size::window_percent(92.)
        };
        let max_width = if self.kind == WorkspaceDialog::Statistics {
            1060.
        } else {
            1220.
        };
        let close = self.origin.clone();
        let close_button = self.origin.clone();
        let close_scrim = self.origin.clone();
        let border = colors::component_border();
        let animate = crate::motion::use_animations_enabled();
        let enter = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.)
                .time(260)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        let enter_offset = if animate {
            (1. - enter.get().value()) * ENTER_SHIFT_PX
        } else {
            0.
        };
        let theme = colors::active_theme();
        let face = match (&self.route, theme.liquid_glass) {
            // The scrolling Mods view already has its own reading surface. Keep the
            // glass tint and rim without filtering the full dialog on each wheel frame.
            (Route::ClusterMods { .. }, true) => rect()
                .corner_radius(rounded(16.))
                .background(colors::page_elevated().with_a(244)),
            (_, true) => {
                let tint =
                    colors::page_elevated().with_a(if theme.colors.light { 198 } else { 188 });
                glass::workspace_panel(rect(), &theme, rounded_px(16.), tint)
            }
            (_, false) => rect()
                .corner_radius(rounded(16.))
                .background(colors::page_elevated()),
        };

        // Menus and confirmation dialogs opened inside the workspace use the regular popup level.
        OverlayPopup::new()
            .overlay_level(7)
            .on_close(move |()| dismiss(&close))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .padding(Gaps::new(
                        NAVBAR_HEIGHT_PX + NAVBAR_GAP_PX,
                        gutter,
                        gutter,
                        gutter,
                    ))
                    .center()
                    // Move the whole panel while its full-window backdrop stays fixed.
                    .offset_y(enter_offset)
                    .background(Color::from_argb(36, 0, 0, 0))
                    .on_press(move |_| dismiss(&close_scrim))
                    .child(
                        face.vertical()
                            .content(Content::Flex)
                            .width(panel_width)
                            .max_width(Size::px(max_width))
                            .height(Size::fill())
                            .max_height(Size::px(930.))
                            .overflow(Overflow::Clip)
                            .shadow(Shadow::from((
                                0.,
                                10.,
                                32.,
                                0.,
                                Color::from_argb(110, 0, 0, 0),
                            )))
                            .border(border_all_color(1., border).alignment(BorderAlignment::Inner))
                            .on_all_press(|event: Event<PressEventData>| event.stop_propagation())
                            .child(
                                rect()
                                    .horizontal()
                                    .content(Content::Flex)
                                    .cross_align(Alignment::Center)
                                    .height(Size::px(header_height))
                                    .width(Size::fill())
                                    .padding(Gaps::new_symmetric(0., HEADER_INSET_PX))
                                    .spacing(12.)
                                    .maybe_child(show_back.then(|| {
                                        Button::new()
                                            .ghost()
                                            .child(Icon::new(IconType::ArrowLeft).size(16.))
                                            .text(t("oneclient.common.back"))
                                            .on_press(move |_| RouterContext::get().go_back())
                                    }))
                                    .child(rect().width(Size::flex(1.)).maybe_child(
                                        (!browsing_in_instance).then(|| {
                                            label()
                                                .text(self.kind.title())
                                                .font_size(text_size(15.))
                                                .font_weight(FontWeight::SEMI_BOLD)
                                                .color(colors::fg_primary())
                                        }),
                                    ))
                                    .child(
                                        Button::new()
                                            .ghost()
                                            .icon()
                                            .width(Size::px(32.))
                                            .height(Size::px(32.))
                                            .alt(t("oneclient.layout.workspace.close"))
                                            .tooltip(t("oneclient.common.close"))
                                            .on_press(move |_| dismiss(&close_button))
                                            .child(Icon::new(IconType::XClose).size(16.)),
                                    ),
                            )
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::px(1.))
                                    .background(border),
                            )
                            .child(
                                rect()
                                    .width(Size::fill())
                                    .height(Size::flex(1.))
                                    .overflow(Overflow::Clip)
                                    .child(RoutedPage(self.route.clone())),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_browse_and_package_remain_in_the_workspace() {
        let browse = Route::Browser {
            cluster_id: 42,
            package_type: "mod".into(),
            pick_cluster: false,
            in_instance: true,
        };
        let package = Route::BrowserPackage {
            cluster_id: 42,
            package_type: "mod".into(),
            package_id: "modrinth-example".into(),
            add_to_cluster: false,
            in_instance: true,
        };

        for route in [browse, package] {
            let decoded = route.to_string().parse::<Route>().unwrap();
            assert_eq!(decoded, route);
            assert_eq!(
                WorkspaceDialog::for_route(&decoded),
                Some(WorkspaceDialog::Instance)
            );
        }

        let old_browse = "/app/browser/42/mod?pick_cluster=true"
            .parse::<Route>()
            .unwrap();
        let old_package = "/app/browser/42/mod/package/example?add_to_cluster=false"
            .parse::<Route>()
            .unwrap();
        assert_eq!(WorkspaceDialog::for_route(&old_browse), None);
        assert_eq!(WorkspaceDialog::for_route(&old_package), None);
    }
}
