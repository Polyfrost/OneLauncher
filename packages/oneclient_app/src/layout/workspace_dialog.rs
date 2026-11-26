use freya::{animation::*, prelude::*, router::*};

use crate::Route;
use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::i18n::t;
use crate::theme::{colors, glass, rounded, rounded_px, text_size};

use super::cluster_shell::InstanceHeader;
use crate::ui::border_all_color;

// The navbar controls extend 68 px from the top of the window.
const NAVBAR_CLEARANCE: f32 = 68.;
const ENTER_SHIFT_PX: f32 = 32.;
pub(super) const CONTENT_TOP: f32 = 16.;
/// Statistics centers its body at this width
const STATISTICS_MAX_WIDTH: f32 = 1060.;

#[derive(Clone, Copy)]
pub(super) struct WorkspaceLayout {
    pub width: f32,
    pub height: f32,
    pub top: f32,
    pub padding: f32,
    pub header_height: f32,
    pub rail_width: f32,
    pub row_height: f32,
    pub row_gap: f32,
}

impl WorkspaceLayout {
    pub fn current() -> Self {
        let window = *Platform::get().current_window().root_size.read();
        Self::for_size(window.width, window.height)
    }

    pub fn for_size(width: f32, height: f32) -> Self {
        let gutter = (16. + (width - 800.) * 0.02).clamp(16., 40.);
        let dialog_width = (width - 2. * gutter).min(1360.);
        // equal space above and below, so the dialog is centered in the window at every size; the top
        // gap still clears the navbar controls
        let dialog_height = (height - 2. * NAVBAR_CLEARANCE).min(960.);
        let padding_t = ((dialog_width - 760.) / 600.).clamp(0., 1.);
        let header_t = ((height - 600.) / 500.).clamp(0., 1.);
        let rail_t = ((height - 700.) / 300.).clamp(0., 1.);
        Self {
            width: dialog_width,
            height: dialog_height,
            top: (height - dialog_height) / 2.,
            padding: ((16. + 8. * padding_t) / 4.).round() * 4.,
            // tall enough for the instance header's 40 px cover and two-line name to breathe
            header_height: 64. + 8. * header_t,
            rail_width: (0.17 * dialog_width).clamp(196., 224.),
            row_height: (32. + 4. * rail_t).round(),
            row_gap: 2. + 2. * rail_t,
        }
    }

    /// The page area right of the rail: P left of the rail, the rail, P between, P on the right
    pub fn pane_width(self) -> f32 {
        (self.width - 3. * self.padding - self.rail_width).max(0.)
    }

    /// The page area below the header: CONTENT_TOP above, P below
    pub fn pane_height(self) -> f32 {
        (self.height - self.header_height - 1. - CONTENT_TOP - self.padding).max(0.)
    }

    /// Left inset of the Statistics body, so its header title starts on the same edge
    pub fn statistics_inset(self) -> f32 {
        self.padding.max((self.width - STATISTICS_MAX_WIDTH) / 2.)
    }

    pub(super) fn header(self) -> Rect {
        rect()
            .horizontal()
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .width(Size::fill())
            .height(Size::px(self.header_height))
            .padding(Gaps::new_symmetric(0., self.padding))
    }
}

pub(super) fn header_divider() -> Rect {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(colors::component_border())
}

pub(super) fn close_button() -> Button {
    let origin = use_app_page();
    Button::new()
        .ghost()
        .icon()
        .width(Size::px(32.))
        .height(Size::px(32.))
        .alt(t("oneclient.layout.workspace.close"))
        .tooltip(t("oneclient.common.close"))
        .on_press(move |_| dismiss(&origin))
        .child(Icon::new(IconType::XClose).size(16.))
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
        let layout = WorkspaceLayout::current();
        let page = RoutedPage(self.route.clone());
        let contents = match &self.route {
            Route::Browser {
                cluster_id,
                in_instance: true,
                ..
            }
            | Route::BrowserPackage {
                cluster_id,
                in_instance: true,
                ..
            } => rect()
                .vertical()
                .content(Content::Flex)
                .width(Size::fill())
                .height(Size::fill())
                .child(InstanceHeader {
                    cluster_id: *cluster_id,
                    show_back: RouterContext::get().can_go_back(),
                })
                .child(header_divider())
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::flex(1.))
                        .child(page),
                )
                .into_element(),
            Route::Stats {} => statistics(layout, page).into_element(),
            _ => page.into_element(),
        };
        let close = self.origin.clone();
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
        let face = if theme.liquid_glass {
            let tint = colors::page_elevated().with_a(if theme.colors.light { 198 } else { 188 });
            glass::workspace_panel(rect(), &theme, rounded_px(16.), tint)
        } else {
            rect()
                .corner_radius(rounded(16.))
                .background(colors::page_elevated())
        };

        // Menus and confirmation dialogs opened inside the workspace use the regular popup level.
        OverlayPopup::new()
            .overlay_level(7)
            .on_close(move |()| dismiss(&close))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .vertical()
                    .cross_align(Alignment::Center)
                    .padding(Gaps::new(layout.top, 0., 0., 0.))
                    .background(Color::from_argb(116, 0, 0, 0))
                    .on_press(move |_| dismiss(&close_scrim))
                    .child(
                        face.vertical()
                            .content(Content::Flex)
                            .width(Size::px(layout.width))
                            .height(Size::px(layout.height))
                            .offset_y(enter_offset)
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
                            .child(contents),
                    ),
            )
    }
}

fn statistics(layout: WorkspaceLayout, page: RoutedPage) -> Rect {
    rect()
        .vertical()
        .content(Content::Flex)
        .width(Size::fill())
        .height(Size::fill())
        .child(
            layout
                .header()
                .padding(Gaps::new(0., layout.padding, 0., layout.statistics_inset()))
                .spacing(12.)
                .child(
                    label()
                        .text(t("oneclient.stats.title"))
                        .font_size(text_size(18.))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(colors::fg_primary()),
                )
                .child(
                    label()
                        .text(t("oneclient.stats.description"))
                        .width(Size::flex(1.))
                        .max_lines(1)
                        .text_overflow(TextOverflow::Ellipsis)
                        .font_size(text_size(12.))
                        .color(crate::view::app::settings::readable_secondary()),
                )
                .child(close_button()),
        )
        .child(header_divider())
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::flex(1.))
                .cross_align(Alignment::Center)
                .padding(Gaps::new(
                    CONTENT_TOP,
                    layout.padding,
                    layout.padding,
                    layout.padding,
                ))
                .child(
                    rect()
                        .width(Size::fill())
                        .max_width(Size::px(STATISTICS_MAX_WIDTH))
                        .height(Size::fill())
                        .child(page),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_frame_follows_window_size_and_is_always_centered() {
        let cases = [
            (800., 600., 768., 464., 68.),
            (1024., 700., 983.04, 564., 68.),
            (1280., 800., 1228.8, 664., 68.),
            (1600., 960., 1360., 824., 68.),
            (2048., 1104., 1360., 960., 72.),
        ];
        for (width, height, expected_width, expected_height, expected_top) in cases {
            let layout = WorkspaceLayout::for_size(width, height);
            assert!((layout.width - expected_width).abs() < 0.01);
            assert!((layout.height - expected_height).abs() < 0.01);
            assert!((layout.top - expected_top).abs() < 0.01);
            // the same gap below as above, and never under the navbar controls
            assert!((height - layout.top - layout.height - layout.top).abs() < 0.01);
            assert!(layout.top >= NAVBAR_CLEARANCE);
        }
        assert!(
            WorkspaceLayout::for_size(1000., 760.).width
                > WorkspaceLayout::for_size(999., 759.).width
        );
    }

    #[test]
    fn workspace_padding_header_and_rail_follow_their_size_formulas() {
        let small = WorkspaceLayout::for_size(800., 600.);
        let middle = WorkspaceLayout::for_size(1280., 850.);
        let large = WorkspaceLayout::for_size(2048., 1104.);

        assert_eq!((small.padding, small.rail_width), (16., 196.));
        assert_eq!(middle.padding, 24.);
        assert!((middle.rail_width - 208.896).abs() < 0.01);
        assert_eq!((large.padding, large.rail_width), (24., 224.));
        assert_eq!(WorkspaceLayout::for_size(1024., 700.).padding, 20.);
        assert_eq!(
            (small.header_height, small.row_height, small.row_gap),
            (64., 32., 2.)
        );
        assert_eq!(
            (middle.header_height, middle.row_height, middle.row_gap),
            (68., 34., 3.)
        );
        assert_eq!(
            (large.header_height, large.row_height, large.row_gap),
            (72., 36., 4.)
        );
    }

    #[test]
    fn pane_and_statistics_insets_follow_the_frame() {
        let small = WorkspaceLayout::for_size(800., 600.);
        // 768 wide: P 16 three times and a 196 rail; 464 tall: a 64 header, its divider,
        // 16 above the page and P below
        assert_eq!(small.pane_width(), 524.);
        assert_eq!(small.pane_height(), 367.);
        // Statistics lines its title up with the centered body
        assert_eq!(small.statistics_inset(), 16.);
        assert!((WorkspaceLayout::for_size(1280., 800.).statistics_inset() - 84.4).abs() < 0.01);
        let full = WorkspaceLayout::for_size(2048., 1104.);
        assert_eq!(full.statistics_inset(), 150.);
        assert_eq!(full.pane_width(), 1064.);
    }

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
