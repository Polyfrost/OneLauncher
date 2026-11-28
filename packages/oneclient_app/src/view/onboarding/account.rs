use freya::prelude::*;
use freya::query::MutationStateData;
use oneclient_auth::MinecraftAccount;

use crate::components::{Avatar, Button, Icon, IconType, TextInput, use_microsoft_login};
use crate::hooks::{
    AddOfflineAccountKeys, try_default_account, use_add_offline_account, use_current_account,
};
use crate::i18n::{t, tf};
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

        let account = try_default_account(&account_query);
        let has_account = account.is_some();

        let content = rect()
            .vertical()
            .width(Size::fill())
            .spacing(24.)
            .child(step_heading(
                t("oneclient.onboarding.account.title"),
                t("oneclient.onboarding.account.subtitle"),
            ))
            .child(match &account {
                Some(account) => account_preview(account).into_element(),
                None => {
                    let start = msa.clone();
                    rect()
                        .vertical()
                        .spacing(20.)
                        .child(sign_in_card(msa.pending, msa.error.clone(), move |_| {
                            start.start()
                        }))
                        .child(OfflineAccountCard)
                        .into_element()
                }
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

/// Inline offline account creation so someone without a Microsoft account can
/// finish onboarding too the created account becomes the default immediately
#[derive(PartialEq)]
struct OfflineAccountCard;

impl Component for OfflineAccountCard {
    fn render(&self) -> impl IntoElement {
        let name = use_state(String::new);
        let add = use_add_offline_account();

        let trimmed = name.read().trim().to_string();
        // Vanilla profiles can never carry such a name so online servers will
        // reject it, but it is still useful for local testing
        let warning = (!trimmed.is_empty())
            .then(|| oneclient_auth::offline_username_warning(&trimmed))
            .flatten();
        let error = match &*add.read().state() {
            MutationStateData::Settled { res: Err(err), .. } => Some(err.to_string()),
            MutationStateData::Loading {
                res: Some(Err(err)),
            } => Some(err.to_string()),
            _ => None,
        };

        let on_confirm = move |_| {
            let trimmed = name.peek().trim().to_string();
            if trimmed.is_empty() {
                return;
            }
            add.mutate(AddOfflineAccountKeys { username: trimmed });
        };

        rect()
            .vertical()
            .spacing(8.)
            .cross_align(Alignment::Start)
            .child(
                label()
                    .text(t("oneclient.onboarding.account.offline_title"))
                    .font_size(text_size(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_secondary()),
            )
            .child(
                rect()
                    .horizontal()
                    .spacing(8.)
                    .cross_align(Alignment::Center)
                    .child(
                        TextInput::new(name)
                            .width(Size::px(220.))
                            .placeholder(t("oneclient.onboarding.account.offline_placeholder")),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .enabled(!trimmed.is_empty())
                            .on_press(on_confirm)
                            .child(Icon::new(IconType::Plus).size(14.))
                            .text(t("oneclient.onboarding.account.offline_create")),
                    ),
            )
            .map(warning, |el, warning| {
                el.child(
                    label()
                        .text(tf(
                            "oneclient.settings.accounts.offline.username_warning",
                            &[&warning],
                        ))
                        .font_size(text_size(11.))
                        .width(Size::fill())
                        .color(colors::fg_secondary()),
                )
            })
            .map(error, |el, msg| {
                el.child(
                    label()
                        .text(msg)
                        .font_size(text_size(11.))
                        .width(Size::fill())
                        .color(colors::danger()),
                )
            })
            .into_element()
    }
}
