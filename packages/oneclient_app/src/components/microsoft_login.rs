use freya::prelude::*;
use freya::query::{MutationCapability, MutationStateData, UseMutation};
use freya::text_edit::Clipboard;
use oneclient_core::LauncherError;
use oneclient_core::auth::{AuthErrorGuidance, MicrosoftLoginSession};
use oneclient_core::notification::MicrosoftLoginStatus;

use crate::components::{Button, Icon, IconType, OverlayPopup};
use crate::hooks::{
    CancelMicrosoftLoginKeys, login_code_already_handled, reset_login_code_dedup,
    use_begin_microsoft_login, use_cancel_microsoft_login, use_finish_microsoft_login,
    use_microsoft_login_status,
};
use crate::platform;
use crate::theme::colors;
use crate::ui::border_all_color;

#[derive(Clone)]
pub struct MicrosoftLogin {
    begin: UseMutation<crate::hooks::BeginMicrosoftLoginMutation>,
    cancel: UseMutation<crate::hooks::CancelMicrosoftLoginMutation>,
    pending_login: State<Option<MicrosoftLoginSession>>,
    cancelled: State<bool>,
    status: Option<MicrosoftLoginStatus>,
    locked: bool,
    pub pending: bool,
    pub error: Option<String>,
    error_guidance: Option<AuthErrorGuidance>,
}

impl MicrosoftLogin {
    pub fn start(&self) {
        let mut cancelled = self.cancelled;
        cancelled.set(false);
        self.begin.mutate(());
    }

    fn cancel(&self) {
        if self.locked {
            return;
        }
        if let Some(session) = self.pending_login.peek().clone() {
            self.cancel.mutate(CancelMicrosoftLoginKeys {
                state_token: session.dedupe_key().to_string(),
            });
        }

        reset_login_code_dedup();

        let mut cancelled = self.cancelled;
        let mut pending_login = self.pending_login;
        cancelled.set(true);
        pending_login.set(None);
    }

    pub fn popup(&self) -> Option<impl IntoElement> {
        let handle = self.clone();
        self.pending_login.read().clone().map(move |login| {
            microsoft_dialog(
                login,
                handle.clone(),
                handle.status.clone(),
                handle.error.clone(),
                handle.error_guidance.clone(),
            )
        })
    }
}

pub fn use_microsoft_login() -> MicrosoftLogin {
    let begin = use_begin_microsoft_login();
    let finish = use_finish_microsoft_login();
    let cancel = use_cancel_microsoft_login();
    let mut pending_login = use_state(|| None::<MicrosoftLoginSession>);
    let cancelled = use_state(|| false);
    let status = use_microsoft_login_status();

    use_side_effect(move || {
        let session = match &*begin.read().state() {
            MutationStateData::Settled {
                res: Ok(session), ..
            } => Some(session.clone()),
            _ => None,
        };
        let Some(session) = session else { return };
        if login_code_already_handled(session.dedupe_key()) {
            return;
        }

        // open the browser immediately
        platform::open_url(session.auth_url());
        finish.mutate(session.clone());
        pending_login.set(Some(session));
    });

    use_side_effect(move || {
        if matches!(
            &*finish.read().state(),
            MutationStateData::Settled { res: Ok(_), .. }
        ) && pending_login.peek().is_some()
        {
            pending_login.set(None);
        }
    });

    // is true once the flow has some status updates (just to ensure the user has seen the popup) and the flow is still in progress
    let locked = status.as_ref().is_some_and(|s| s.current > 0);
    let in_flight = begin.read().state().is_loading() || finish.read().state().is_loading();
    let pending = in_flight && !*cancelled.read();
    let error_info = mutation_err_info(&finish).or_else(|| mutation_err_info(&begin));
    let (error, error_guidance) = match error_info {
        Some(info) => (Some(info.message), info.guidance),
        None => (None, None),
    };

    MicrosoftLogin {
        begin,
        cancel,
        pending_login,
        cancelled,
        status,
        locked,
        pending,
        error,
        error_guidance,
    }
}

struct AuthErrorInfo {
    message: String,
    guidance: Option<AuthErrorGuidance>,
}

fn mutation_err_info<M>(mutation: &UseMutation<M>) -> Option<AuthErrorInfo>
where
    M: MutationCapability<Err = LauncherError>,
{
    let read = mutation.read();
    match &*read.state() {
        MutationStateData::Settled { res: Err(err), .. }
        | MutationStateData::Loading {
            res: Some(Err(err)),
        } => Some(AuthErrorInfo {
            message: err.to_string(),
            guidance: err.auth_guidance(),
        }),
        _ => None,
    }
}

fn microsoft_dialog(
    login: MicrosoftLoginSession,
    handle: MicrosoftLogin,
    status: Option<MicrosoftLoginStatus>,
    error: Option<String>,
    guidance: Option<AuthErrorGuidance>,
) -> impl IntoElement {
    let close = handle.clone();
    login_dialog(
        login.auth_url().to_string(),
        login.user_code().to_string(),
        login.verification_uri().to_string(),
        handle.locked,
        status,
        error,
        guidance,
        move || close.cancel(),
    )
}

#[expect(clippy::too_many_arguments)]
pub(crate) fn login_dialog(
    auth_url: String,
    user_code: String,
    verification_uri: String,
    locked: bool,
    status: Option<MicrosoftLoginStatus>,
    error: Option<String>,
    guidance: Option<AuthErrorGuidance>,
    on_close: impl FnMut() + Clone + 'static,
) -> impl IntoElement {
    let mut close_scrim = on_close.clone();
    let mut close_cancel = on_close;
    OverlayPopup::new()
        .on_close(move |()| close_scrim())
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(420.))
                        .max_width(Size::window_percent(90.))
                        .max_height(Size::window_percent(90.))
                        .cross_align(Alignment::Center)
                        .spacing(18.)
                        .padding(Gaps::new_all(28.))
                        .corner_radius(CornerRadius::new_all(16.))
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text("Sign in to Microsoft")
                                .font_size(18.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            ScrollView::new()
                                .width(Size::fill())
                                .height(Size::auto())
                                .max_height(Size::window_percent(70.))
                                .child(
                                    rect()
                                        .vertical()
                                        .width(Size::fill())
                                        .cross_align(Alignment::Center)
                                        .spacing(18.)
                                        .child(browser_dialog_body(auth_url))
                                        .child(dialog_divider())
                                        .child(device_code_dialog_body(
                                            user_code,
                                            verification_uri,
                                        ))
                                        .child(status_row(status, error))
                                        .maybe_child(guidance.map(guidance_block)),
                                ),
                        )
                        .maybe_child((!locked).then(|| {
                            Button::new()
                                .ghost()
                                .on_press(move |_| close_cancel())
                                .text("Cancel")
                                .into_element()
                        })),
                ),
        )
        .into_element()
}

fn dialog_divider() -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .spacing(10.)
        .child(
            rect()
                .width(Size::flex(1.0))
                .height(Size::px(1.))
                .background(colors::component_border()),
        )
        .child(
            label()
                .text("or")
                .font_size(11.)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .width(Size::flex(1.0))
                .height(Size::px(1.))
                .background(colors::component_border()),
        )
        .into_element()
}

fn status_row(status: Option<MicrosoftLoginStatus>, error: Option<String>) -> impl IntoElement {
    if let Some(error) = error {
        return rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(6.)
            .child(
                Icon::new(IconType::AlertTriangle)
                    .size(13.)
                    .color(colors::danger()),
            )
            .child(label().text(error).font_size(12.).color(colors::danger()))
            .into_element();
    }

    let text = match &status {
        Some(status) if status.total > 0 && status.current > 0 => {
            format!("{} ({}/{})", status.label, status.current, status.total)
        }
        _ => "Waiting for you to finish signing in...".to_string(),
    };

    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .child(
            Icon::new(IconType::Loading02)
                .size(13.)
                .color(colors::brand()),
        )
        .child(
            label()
                .text(text)
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn guidance_block(guidance: AuthErrorGuidance) -> impl IntoElement {
    let mut steps = rect()
        .vertical()
        .width(Size::fill())
        .spacing(6.)
        .child(
            label()
                .text("What you can do:")
                .font_size(12.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(colors::fg_primary()),
        );

    for (index, step) in guidance.steps_to_fix.into_iter().enumerate() {
        steps = steps.child(
            rect()
                .horizontal()
                .width(Size::fill())
                .spacing(6.)
                .child(
                    label()
                        .text(format!("{}.", index + 1))
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                )
                .child(
                    label()
                        .text(step)
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        );
    }

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(10.)
        .padding(Gaps::new_all(14.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::component_bg())
        .border(border_all_color(1., colors::component_border()))
        .child(
            label()
                .text(guidance.what_happened)
                .font_size(12.)
                .color(colors::fg_primary()),
        )
        .child(steps)
        .into_element()
}

fn browser_dialog_body(auth_url: String) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            label()
                .text("We opened the Microsoft sign-in page in your browser. Finish there and you'll be brought back automatically.")
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .child(
            Button::new()
                .primary()
                .on_press(move |_| platform::open_url(&auth_url))
                .child(Icon::new(IconType::LinkExternal01).size(16.))
                .text("Open in browser again"),
        )
        .into_element()
}

fn device_code_dialog_body(code: String, verification_uri: String) -> impl IntoElement {
    let copy_code = code.clone();
    rect()
        .vertical()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(12.)
        .child(
            label()
                .text("Or enter this code at the Microsoft sign-in page:")
                .font_size(13.)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .width(Size::fill())
                .center()
                .padding(Gaps::new_symmetric(20., 16.))
                .corner_radius(CornerRadius::new_all(14.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::brand()))
                .child(
                    label()
                        .text(code)
                        .font_size(48.)
                        .font_weight(FontWeight::BOLD)
                        .color(colors::fg_primary()),
                ),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::Center)
                .spacing(8.)
                .child(
                    Button::new()
                        .secondary()
                        .on_press(move |_| {
                            if let Err(err) = Clipboard::set(copy_code.clone()) {
                                tracing::warn!("clipboard copy failed: {err:?}");
                            }
                        })
                        .child(Icon::new(IconType::Copy01).size(16.))
                        .text("Copy code"),
                )
                .child(
                    Button::new()
                        .primary()
                        .on_press(move |_| platform::open_url(&verification_uri))
                        .child(Icon::new(IconType::LinkExternal01).size(16.))
                        .text("Open in browser"),
                ),
        )
        .into_element()
}
