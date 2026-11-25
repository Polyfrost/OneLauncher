use freya::prelude::*;
use freya::query::{MutationStateData, UseMutation};

use crate::components::{Button, Icon, IconType, OverlayPopup, TextInput};
use crate::hooks::{
    AddMicrosoftAccountFromTokenKeys, AddMicrosoftAccountFromTokenMutation, mutation_is_running,
    mutation_ok, use_add_microsoft_account_from_token,
};
use crate::i18n::t;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

#[derive(Clone)]
pub(crate) struct MicrosoftTokenLogin {
    mutation: UseMutation<AddMicrosoftAccountFromTokenMutation>,
    open: State<bool>,
    token: State<String>,
    pub pending: bool,
    pub error: Option<String>,
}

impl MicrosoftTokenLogin {
    pub(crate) fn start(&self) {
        let mut open = self.open;
        open.set(true);
    }

    fn close(&self) {
        let mut open = self.open;
        open.set(false);
    }

    fn sign_in(&self) {
        let token = self.token.peek().trim().to_string();
        if token.is_empty() {
            return;
        }
        self.mutation
            .mutate(AddMicrosoftAccountFromTokenKeys { token });
    }

    pub(crate) fn popup(&self) -> Option<impl IntoElement> {
        let open = *self.open.read();
        if !open {
            return None;
        }
        Some(token_dialog(self.clone()))
    }
}

pub(crate) fn use_microsoft_token_login() -> MicrosoftTokenLogin {
    let mutation = use_add_microsoft_account_from_token();
    let open = use_state(|| false);
    let token = use_state(String::new);

    let mut close_on_success = open;
    use_side_effect(move || {
        if mutation_ok(&mutation).is_some() {
            close_on_success.set(false);
        }
    });

    let pending = mutation_is_running(&mutation);
    let error = {
        let read = mutation.read();
        match &*read.state() {
            MutationStateData::Settled { res: Err(err), .. }
            | MutationStateData::Loading {
                res: Some(Err(err)),
                ..
            } => Some(err.to_string()),
            _ => None,
        }
    };

    MicrosoftTokenLogin {
        mutation,
        open,
        token,
        pending,
        error,
    }
}

fn token_dialog(login: MicrosoftTokenLogin) -> impl IntoElement {
    let close_scrim = login.clone();
    let close_cancel = login.clone();
    let submit = login.clone();
    let token_field = login.token;
    // `read` subscribes this component to edits; `peek` would leave the
    // Sign in button disabled until some other state change forced a render.
    let token_empty = login.token.read().trim().is_empty();

    OverlayPopup::new()
        .on_close(move |()| close_scrim.close())
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
                        .max_height(Size::window_percent(90.))
                        .spacing(16.)
                        .padding(Gaps::new_all(28.))
                        .corner_radius(rounded(16.))
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text(t("oneclient.components.microsoft_token_login.title"))
                                .font_size(text_size(18.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(t("oneclient.components.microsoft_token_login.description"))
                                .font_size(text_size(12.))
                                .color(colors::fg_secondary()),
                        )
                        .child(
                            TextInput::new(token_field)
                                .multiline(true)
                                .auto_focus(true)
                                .height(Size::px(140.))
                                .placeholder(t(
                                    "oneclient.components.microsoft_token_login.placeholder",
                                )),
                        )
                        .maybe_child(login.error.clone().map(|message| {
                            rect()
                                .horizontal()
                                .cross_align(Alignment::Center)
                                .spacing(6.)
                                .child(
                                    Icon::new(IconType::AlertTriangle)
                                        .size(13.)
                                        .color(colors::danger()),
                                )
                                .child(
                                    label()
                                        .text(message)
                                        .font_size(text_size(12.))
                                        .color(colors::danger()),
                                )
                                .into_element()
                        }))
                        .child(
                            rect()
                                .horizontal()
                                .main_align(Alignment::End)
                                .spacing(10.)
                                .child(
                                    Button::new()
                                        .ghost()
                                        .on_press(move |_| close_cancel.close())
                                        .text(t("oneclient.common.cancel")),
                                )
                                .child(
                                    Button::new()
                                        .primary()
                                        .enabled(!login.pending && !token_empty)
                                        .on_press(move |_| submit.sign_in())
                                        .text(if login.pending {
                                            t("oneclient.components.microsoft_token_login.signing_in")
                                        } else {
                                            t("oneclient.components.microsoft_token_login.sign_in")
                                        }),
                                ),
                        ),
                ),
        )
        .into_element()
}
