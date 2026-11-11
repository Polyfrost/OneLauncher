use freya::prelude::*;
use oneclient_common::GameLoader;
use oneclient_db::models::ClusterKind;

use crate::components::{Button, modal};
use crate::hooks::{ClusterAction, use_cluster_mutation, use_dispatch, use_invite_join};
use crate::i18n::{t, tf};
use crate::invite_join::custom_loader;
use crate::state::InviteJoinPrompt;
use crate::theme::{colors, text_size};
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
            Some(InviteJoinPrompt::Pick {
                invite_id,
                clusters,
            }) => ClusterPickerModal {
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
                unlinked,
            }) => {
                let game = format!("{version} {}", loader.real_name());
                let creating = dispatch.clone();
                let custom = dispatch.clone();
                let custom_version = version.clone();
                let offer_custom = custom_loader(loader) && !unlinked;

                let recommended = (oneclient || unlinked).then(|| {
                    Button::new()
                        .primary()
                        .width(Size::fill())
                        .on_press(move |_| {
                            creating.clear_invite_join();
                            if unlinked {
                                creating.link_polyplus_and_join(invite_id);
                            } else {
                                let action = oneclient_cluster(version.clone(), loader);
                                creating.create_cluster_and_join(mutation, action, invite_id);
                            }
                        })
                        .text(if unlinked {
                            t("oneclient.components.invite_join.add_polyplus")
                        } else {
                            t("oneclient.components.invite_join.create_oneclient")
                        })
                        .into_element()
                });

                let (title, body) = if unlinked {
                    (
                        tf("oneclient.components.invite_join.unlinked_title", &[&game]),
                        t("oneclient.components.invite_join.unlinked_body"),
                    )
                } else {
                    (
                        tf("oneclient.components.invite_join.none_title", &[&game]),
                        if oneclient {
                            t("oneclient.components.invite_join.none_body")
                        } else if custom_loader(loader) {
                            t("oneclient.components.invite_join.none_body_unsupported")
                        } else {
                            t("oneclient.components.invite_join.none_body_no_custom")
                        },
                    )
                };

                dialog(
                    title,
                    body.to_string(),
                    on_close.clone(),
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(8.)
                        .maybe_child(recommended)
                        .maybe_child(offer_custom.then(|| {
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
    modal(
        DIALOG_WIDTH_PX,
        on_close,
        rect()
            .child(
                label()
                    .text(title)
                    .width(Size::fill())
                    .font_size(text_size(16.))
                    .font_weight(FontWeight::SEMI_BOLD)
                    .color(colors::fg_primary()),
            )
            .child(
                label()
                    .text(body)
                    .width(Size::fill())
                    .font_size(text_size(13.))
                    .color(colors::fg_secondary()),
            )
            .child(actions),
    )
}
