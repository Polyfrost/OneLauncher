use freya::prelude::*;
use oneclient_content::packages::ProviderId;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::use_dispatch;
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

#[derive(Clone, PartialEq)]
pub struct PendingBundledInstall {
    pub cluster_id: i64,
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
    pub project_name: String,
    pub version_label: String,
    pub bundled_version: Option<String>,
}

pub fn bundled_install_body(pending: &PendingBundledInstall) -> String {
    let project = &pending.project_name;
    let incoming = &pending.version_label;

    match &pending.bundled_version {
        Some(bundled) => format!(
            "{project} comes with this cluster's bundle, pinned to {bundled}. Installing {incoming} overrides the bundle's copy and can break other mods in the cluster or stop the game from starting."
        ),
        None => format!(
            "{project} comes with this cluster's bundle. Installing {incoming} overrides the bundle's copy and can break other mods in the cluster or stop the game from starting."
        ),
    }
}

#[derive(PartialEq)]
pub struct BundledInstallWarning {
    pub pending: State<Option<PendingBundledInstall>>,
}

impl Component for BundledInstallWarning {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mut pending = self.pending;

        let Some(install) = pending.read().clone() else {
            return rect().into_element();
        };

        let body = bundled_install_body(&install);

        OverlayPopup::new()
            .on_close(move |_| pending.set(None))
            .child(
                rect()
                    .width(Size::window_percent(100.))
                    .height(Size::window_percent(100.))
                    .center()
                    .child(
                        rect()
                            .vertical()
                            .width(Size::px(440.))
                            .max_width(Size::window_percent(90.))
                            .spacing(14.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(CornerRadius::new_all(14.))
                            .background(CARD_BG)
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                rect()
                                    .horizontal()
                                    .cross_align(Alignment::Center)
                                    .spacing(10.)
                                    .child(
                                        Icon::new(IconType::AlertTriangle)
                                            .size(20.)
                                            .color(colors::code_warn()),
                                    )
                                    .child(
                                        label()
                                            .text("Not the bundled version")
                                            .font_size(16.)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .color(colors::fg_primary()),
                                    ),
                            )
                            .child(
                                label()
                                    .text(body)
                                    .font_size(12.)
                                    .max_lines(8)
                                    .width(Size::fill())
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .secondary()
                                            .on_press(move |_| pending.set(None))
                                            .text("Cancel"),
                                    )
                                    .child(
                                        Button::new()
                                            .danger()
                                            .on_press(move |_| {
                                                dispatch.install_package(
                                                    install.cluster_id,
                                                    install.provider,
                                                    install.project_id.clone(),
                                                    install.version_id.clone(),
                                                );
                                                pending.set(None);
                                            })
                                            .child(Icon::new(IconType::Download01).size(14.))
                                            .text("Install anyway"),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(bundled_version: Option<&str>) -> PendingBundledInstall {
        PendingBundledInstall {
            cluster_id: 1,
            provider: ProviderId::Modrinth,
            project_id: "sodium".to_string(),
            version_id: "v2".to_string(),
            project_name: "Sodium".to_string(),
            version_label: "0.6.0".to_string(),
            bundled_version: bundled_version.map(Into::into),
        }
    }

    #[test]
    fn the_body_names_both_versions() {
        let body = bundled_install_body(&pending(Some("0.5.8")));

        assert!(body.contains("Sodium"));
        assert!(body.contains("0.5.8"));
        assert!(body.contains("0.6.0"));
    }

    #[test]
    fn an_unknown_pin_leaves_the_bundled_version_out() {
        let body = bundled_install_body(&pending(None));

        assert!(body.contains("0.6.0"));
        assert!(!body.contains("pinned to"));
    }
}
