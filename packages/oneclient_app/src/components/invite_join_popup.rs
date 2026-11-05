use freya::prelude::*;
use oneclient_common::GameLoader;
use oneclient_db::models::ClusterKind;

use crate::components::{Button, OVERLAY_MAX_LEVEL, OverlayPopup};
use crate::hooks::{ClusterAction, use_cluster_mutation, use_dispatch, use_invite_join};
use crate::i18n::{t, tf};
use crate::invite_join::custom_loader;
use crate::state::InviteJoinPrompt;
use crate::theme::{colors, rounded};
use crate::ui::border_all_color;
use crate::view::app::{ClusterPickerModal, CreateInstanceModal};

const DIALOG_WIDTH_PX: f32 = 400.;

#[derive(PartialEq)]
pub struct InviteJoinPopup;

impl Component for InviteJoinPopup {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let mutation = use_cluster_mutation();
        let close = dispatch.clone();
        let on_close = EventHandler::new(move |()| close.clear_invite_join());

        match use_invite_join() {
            None => rect().into_element(),
            Some(InviteJoinPrompt::Pick { invite_id, clusters }) => ClusterPickerModal {
                selected: None,
                only: Some(clusters),
                on_close,
                on_pick: EventHandler::new(move |cluster: oneclient_cluster::Cluster| {
                    dispatch.clear_invite_join();
                    dispatch.close_social();
                    dispatch.launch_cluster_for_invite(cluster.id, invite_id);
                }),
            }
            .into_element(),
            Some(InviteJoinPrompt::Create {
                invite_id,
                version,
                loader,
                oneclient,
            }) => {
                let game = format!("{version} {}", loader.real_name());
                let creating = dispatch.clone();
                let custom = dispatch.clone();
                let custom_version = version.clone();

                let recommended = oneclient.then(|| {
                    Button::new()
                        .primary()
                        .width(Size::fill())
                        .on_press(move |_| {
                            let action = oneclient_cluster(version.clone(), loader);
                            creating.clear_invite_join();
                            creating.create_cluster_and_join(mutation, action, invite_id);
                        })
                        .text(t("oneclient.components.invite_join.create_oneclient"))
                        .into_element()
                });

                dialog(
                    tf("oneclient.components.invite_join.none_title", &[&game]),
                    if oneclient {
                        t("oneclient.components.invite_join.none_body").to_string()
                    } else if custom_loader(loader) {
                        t("oneclient.components.invite_join.none_body_unsupported").to_string()
                    } else {
                        t("oneclient.components.invite_join.none_body_no_custom").to_string()
                    },
                    on_close.clone(),
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(8.)
                        .maybe_child(recommended)
                        .maybe_child(custom_loader(loader).then(|| {
                            Button::new()
                                .secondary()
                                .width(Size::fill())
                                .on_press(move |_| {
                                    custom.custom_invite_cluster(
                                        invite_id,
                                        custom_version.clone(),
                                        loader,
                                    );
                                })
                                .text(t("oneclient.components.invite_join.create_custom"))
                                .into_element()
                        }))
                        .child(
                            Button::new()
                                .ghost()
                                .width(Size::fill())
                                .on_press(move |_| on_close.call(()))
                                .text(t("oneclient.common.cancel")),
                        ),
                )
            }
            Some(InviteJoinPrompt::Custom {
                invite_id,
                version,
                loader,
            }) => CreateInstanceModal::new(on_close)
                .prefill(version, loader)
                .join_invite(invite_id)
                .into_element(),
        }
    }
}

fn oneclient_cluster(version: String, loader: GameLoader) -> ClusterAction {
    ClusterAction::CreateInstance {
        kind: ClusterKind::OneClient,
        name: format!("{version} {loader}"),
        mc_version: version,
        mc_loader: loader,
        mc_loader_version: None,
        description: None,
        tags: Vec::new(),
        cover_source: None,
        bundles: None,
        require_bundles: true,
        dedicated: false,
        shared_group: None,
    }
}

fn dialog(title: String, body: String, on_close: EventHandler<()>, actions: Rect) -> Element {
    OverlayPopup::new()
        .overlay_level(OVERLAY_MAX_LEVEL)
        .on_close(move |()| on_close.call(()))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(DIALOG_WIDTH_PX))
                        .max_width(Size::window_percent(90.))
                        .spacing(14.)
                        .padding(Gaps::new_all(20.))
                        .corner_radius(rounded(14.))
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text(title)
                                .width(Size::fill())
                                .font_size(16.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(body)
                                .width(Size::fill())
                                .font_size(13.)
                                .color(colors::fg_secondary()),
                        )
                        .child(actions),
                ),
        )
        .into_element()
}
