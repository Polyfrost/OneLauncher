use freya::prelude::*;
use oneclient_auth::MinecraftAccount;

use crate::components::{
    Avatar, Button, Icon, IconType, use_microsoft_login, use_microsoft_token_login,
};
use crate::hooks::{
    RemoveAccountKeys, try_default_account, use_current_account, use_remove_account,
};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::{colors, text_size};
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

#[derive(PartialEq)]
pub struct OnboardingAccount;

impl Component for OnboardingAccount {
    fn render(&self) -> impl IntoElement {
        let account_query = use_current_account();
        let msa = use_microsoft_login();
        let token_login = use_microsoft_token_login();
        let remove = use_remove_account();

        let account = try_default_account(&account_query);
        let has_account = account.is_some();

        let preview: Option<Element> = account
            .as_ref()
            .map(|account| account_preview(account).into_element());
        let wrong_account: Option<Element> = account.as_ref().map(|account| {
            let remove = remove.clone();
            let id = account.id;
            Button::new()
                .ghost()
                .on_press(move |_| remove.mutate(RemoveAccountKeys { id }))
                .text(t("oneclient.home.wrong_account"))
                .into_element()
        });

        let start = msa.clone();
        let start_token = token_login.clone();
        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(24.)
            .child(step_heading(
                t("oneclient.onboarding.account.title"),
                t("oneclient.onboarding.account.subtitle"),
            ))
            .maybe_child(preview)
            .maybe_child(wrong_account)
            .maybe(account.is_none(), |page| {
                page.child(sign_in_card(
                    msa.pending,
                    msa.error.clone(),
                    move |_| start.start(),
                    move |_| start_token.start(),
                ))
            })
            .into_element();

        let page = onboarding_page(
            onboarding_illustration(IconType::OnboardingAccount),
            content,
            onboarding_nav(
                Some(Route::OnboardingLanguage {}),
                Route::OnboardingBundlePicker {},
                has_account,
            ),
        );

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(page)
            .maybe_child(msa.popup())
            .maybe_child(token_login.popup())
    }
}

fn account_preview(account: &MinecraftAccount) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .spacing(24.)
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
                                .font_size(text_size(16.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(account.id.to_string())
                                .font_size(text_size(12.))
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
    on_token: impl FnMut(Event<PressEventData>) + 'static,
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
                    t("oneclient.onboarding.account.signing_in")
                } else {
                    t("oneclient.onboarding.account.add")
                }),
        )
        .child(
            Button::new()
                .secondary()
                .large()
                .enabled(!pending)
                .on_press(on_token)
                .child(Icon::new(IconType::Key01).size(16.))
                .text(t("oneclient.onboarding.account.token_login")),
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
                        .font_size(text_size(12.))
                        .color(colors::danger()),
                )
                .into_element()
        }))
        .into_element()
}
