use std::sync::Mutex;

use freya::prelude::*;
use freya::query::{MutationCapability, MutationStateData, UseMutation};
use freya::text_edit::Clipboard;
use oneclient_core::auth::{MinecraftAccount, MinecraftLogin};

use crate::components::{Avatar, Button, Icon, IconType, OverlayPopup};
use crate::hooks::{
    try_default_account, use_begin_microsoft_login, use_current_account,
    use_finish_microsoft_login,
};
use crate::platform;
use crate::routes::Route;
use crate::theme::colors;
use crate::ui::border_all_color;
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

static HANDLED_LOGIN_CODE: Mutex<Option<String>> = Mutex::new(None);

#[derive(PartialEq)]
pub struct OnboardingAccount;

impl Component for OnboardingAccount {
    fn render(&self) -> impl IntoElement {
        let account_query = use_current_account();

        let begin = use_begin_microsoft_login();
        let finish = use_finish_microsoft_login();

        let mut pending_login = use_state(|| None::<MinecraftLogin>);

        use_side_effect(move || {
            let login = match &*begin.read().state() {
                MutationStateData::Settled { res: Ok(login), .. } => Some(login.clone()),
                _ => None,
            };
            let Some(login) = login else { return };
            {
                let mut handled = HANDLED_LOGIN_CODE.lock().unwrap();
                if handled.as_deref() == Some(login.user_code.as_str()) {
                    return;
                }
                *handled = Some(login.user_code.clone());
            }
            finish.mutate(login.clone());
            pending_login.set(Some(login));
        });

        use_side_effect(move || {
            if matches!(&*finish.read().state(), MutationStateData::Settled { .. })
                && pending_login.peek().is_some()
            {
                pending_login.set(None);
            }
        });

        let account = try_default_account(&account_query);
        let has_account = account.is_some();
        let pending = begin.read().state().is_loading() || finish.read().state().is_loading();
        let error = mutation_err_text(&finish).or_else(|| mutation_err_text(&begin));

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(24.)
            .child(step_heading(
                "Account",
                "Before you continue, we require you to own a copy of Minecraft: Java Edition.",
            ))
            .child(match &account {
                Some(account) => account_preview(account).into_element(),
                None => sign_in_card(pending, error, move |_| begin.mutate(())).into_element(),
            })
            .into_element();

        let page = onboarding_page(
            onboarding_illustration(IconType::OnboardingAccount),
            content,
            onboarding_nav(
                Some(Route::OnboardingLanguage {}),
                Route::OnboardingBundles {},
                has_account,
            ),
        );

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(page)
            .maybe_child(
                pending_login
                    .read()
                    .clone()
                    .map(|login| microsoft_dialog(login, pending_login)),
            )
    }
}

fn mutation_err_text<M>(mutation: &UseMutation<M>) -> Option<String>
where
    M: MutationCapability,
    M::Err: std::fmt::Display,
{
    match &*mutation.read().state() {
        MutationStateData::Settled { res: Err(err), .. } => Some(err.to_string()),
        MutationStateData::Loading {
            res: Some(Err(err)),
        } => Some(err.to_string()),
        _ => None,
    }
}

fn account_preview(account: &MinecraftAccount) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        // .height(Size::px(260.))
        .spacing(24.)
        // .cross_align(Alignment::Center)
        // .child(
        //     rect()
        //         .width(Size::px(200.))
        //         .height(Size::fill())
        //         .corner_radius(CornerRadius::new_all(16.))
        //         .background(colors::page_elevated())
        //         .border(border_all_color(1., colors::component_border()))
        //         .child(
        //             PlayerModel::new(account.id)
        //                 .yaw(-0.5)
        //                 .width(Size::fill())
        //                 .height(Size::fill()),
        //         ),
        // )
        .child(
            rect()
                .horizontal()
                .spacing(12.)
                .cross_align(Alignment::Center)
                .child(
                    Avatar::new(account.id.to_string())
                        .width(Size::px(48.))
                        .height(Size::px(48.)),
                )
                .child(
                    rect()
                        .vertical()
                        .spacing(4.)
                        .child(
                            label()
                                .text(account.username.clone())
                                .font_size(16.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(account.id.to_string())
                                .font_size(12.)
                                .color(colors::fg_secondary()),
                        ),
                ),
        )
        .into_element()
}

fn sign_in_card(
    pending: bool,
    error: Option<String>,
    on_add: impl FnMut(Event<PressEventData>) + 'static,
) -> impl IntoElement {
    rect()
        .vertical()
        .spacing(12.)
        .cross_align(Alignment::Start)
        .child(
            Button::new()
                .primary()
                .large()
                .enabled(!pending)
                .on_press(on_add)
                .child(Icon::new(IconType::Globe01).size(16.))
                .text(if pending {
                    "Signing in..."
                } else {
                    "Add Account"
                }),
        )
        .maybe_child(error.map(|message| {
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
                        .font_size(12.)
                        .color(colors::danger()),
                )
                .into_element()
        }))
        .into_element()
}

fn microsoft_dialog(
    login: MinecraftLogin,
    mut pending_login: State<Option<MinecraftLogin>>,
) -> impl IntoElement {
    let code = login.user_code.clone();
    let copy_code = code.clone();
    let verification_uri = login.verification_uri.clone();

    OverlayPopup::new()
        .on_close(move |()| pending_login.set(None))
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
                            label()
                                .text("Enter this code on the Microsoft sign-in page:")
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
                        .child(
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
                                        .text("Waiting for you to finish signing in...")
                                        .font_size(12.)
                                        .color(colors::fg_secondary()),
                                ),
                        )
                        .child(
                            Button::new()
                                .ghost()
                                .on_press(move |_| pending_login.set(None))
                                .text("Cancel"),
                        ),
                ),
        )
        .into_element()
}
