use freya::prelude::*;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::theme::colors;
use crate::ui::border_all_color;

const CARD_BG: Color = Color::from_rgb(26, 34, 41);

pub fn use_shared_delete(
    delete: impl Into<EventHandler<(String, String)>>,
) -> (EventHandler<(String, String)>, Option<Element>) {
    let mut pending = use_state(|| None::<(String, String, usize)>);
    let delete = delete.into();

    let request = {
        let delete = delete.clone();
        EventHandler::new_current(move |(name, hash): (String, String)| {
            let delete = delete.clone();
            spawn(async move {
                let clusters = match crate::launcher::state() {
                    Ok(state) => oneclient_core::clusters_sharing_artifact(
                        &hash,
                        &state.services.content(),
                    )
                    .await
                    .unwrap_or_else(|err| {
                        tracing::warn!(%err, "could not check whether this package is shared");
                        None
                    }),
                    Err(_) => None,
                };
                match clusters {
                    Some(clusters) if clusters > 1 => pending.set(Some((name, hash, clusters))),
                    _ => delete.call((name, hash)),
                }
            });
        })
    };

    let dialog = pending.read().clone().map(|(name, hash, clusters)| {
        SharedPackageDeleteDialog {
            name: name.clone(),
            clusters,
            on_cancel: (move |()| pending.set(None)).into(),
            on_confirm: (move |()| {
                delete.call((name.clone(), hash.clone()));
                pending.set(None);
            })
            .into(),
        }
        .into_element()
    });

    (request, dialog)
}

#[derive(PartialEq)]
struct SharedPackageDeleteDialog {
    name: String,
    clusters: usize,
    on_cancel: EventHandler<()>,
    on_confirm: EventHandler<()>,
}

impl Component for SharedPackageDeleteDialog {
    fn render(&self) -> impl IntoElement {
        let title = format!("Delete {} from every cluster?", self.name);
        let body = format!(
            "Resource packs and shaders are shared, so this deletes it from all {} clusters that use it.",
            self.clusters
        );
        let close = self.on_cancel.clone();
        let cancel = self.on_cancel.clone();
        let confirm = self.on_confirm.clone();

        OverlayPopup::new().on_close(move |_| close.call(())).child(
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
                                        .text(title)
                                        .font_size(16.)
                                        .font_weight(FontWeight::SEMI_BOLD)
                                        .color(colors::fg_primary()),
                                ),
                        )
                        .child(
                            label()
                                .text(body)
                                .font_size(12.)
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
                                        .on_press(move |_| cancel.call(()))
                                        .text("Cancel"),
                                )
                                .child(
                                    Button::new()
                                        .danger()
                                        .on_press(move |_| confirm.call(()))
                                        .child(Icon::new(IconType::Trash01).size(14.))
                                        .text("Delete"),
                                ),
                        ),
                ),
        )
    }
}
