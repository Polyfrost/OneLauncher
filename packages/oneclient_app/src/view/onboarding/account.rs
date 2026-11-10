use freya::prelude::*;
use freya::query::{MutationCapability, MutationStateData, UseMutation};
use oneclient_auth::{MinecraftAccount, offline_uuid, validate_offline_username};

use crate::components::{
    Avatar, Button, Icon, IconType, MicrosoftLogin, TextInput, use_microsoft_login,
};
use crate::hooks::{
    AddOfflineAccountKeys, mutation_is_running, mutation_ok, try_default_account,
    use_add_offline_account, use_current_account,
};
use crate::i18n::t;
use crate::routes::Route;
use crate::theme::{colors, text_size};
use crate::ui::divider;
use crate::view::onboarding::{
    onboarding_illustration, onboarding_nav, onboarding_page, step_heading,
};

#[derive(PartialEq)]
pub struct OnboardingAccount;

impl Component for OnboardingAccount {
    fn render(&self) -> impl IntoElement {
        let account_query = use_current_account();
        let msa = use_microsoft_login();
        let add_offline = use_add_offline_account();
        let username = use_state(String::new);

        // No licence and no Microsoft account is needed: the local account is
        // committed straight into the credentials store and the query picks it
        // up, but the mutation's own result covers the frame before it reloads.
        let account = try_default_account(&account_query).or_else(|| mutation_ok(&add_offline));
        let has_account = account.is_some();
        let pending = mutation_is_running(&add_offline);
        let error = mutation_err_text(&add_offline);

        let add = add_offline.clone();
        let on_play = move |_: Event<PressEventData>| {
            let name = username.peek().trim().to_string();
            if validate_offline_username(&name).is_err() {
                return;
            }
            add.mutate(AddOfflineAccountKeys { username: name });
        };

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
                None => offline_form(username, pending, error, on_play, msa.clone()).into_element(),
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

fn offline_form(
    username: State<String>,
    pending: bool,
    error: Option<String>,
    on_play: impl FnMut(Event<PressEventData>) + 'static,
    msa: MicrosoftLogin,
) -> impl IntoElement {
    let name = username.read().trim().to_string();
    let valid = validate_offline_username(&name).is_ok();
    let uuid = valid.then(|| offline_uuid(&name).to_string());
    let msa_pending = msa.pending;
    let msa_error = msa.error.clone();

    let start_msa = msa;
    let on_msa = move |_: Event<PressEventData>| start_msa.start();

    rect()
        .vertical()
        .width(Size::fill())
        .spacing(16.)
        .cross_align(Alignment::Start)
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(6.)
                .child(field_label(t("oneclient.onboarding.account.username")))
                .child(TextInput::new(username).placeholder(t(
                    "oneclient.onboarding.account.username_placeholder",
                ))),
        )
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(6.)
                .child(field_label(t("oneclient.onboarding.account.uuid")))
                .child(
                    label()
                        .text(uuid.unwrap_or_else(|| "-".to_string()))
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary()),
                ),
        )
        .maybe_child((!valid && !name.is_empty()).then(|| {
            hint_line(
                IconType::AlertTriangle,
                t("oneclient.onboarding.account.username_hint").to_string(),
                colors::fg_secondary(),
            )
        }))
        .child(
            Button::new()
                .primary()
                .large()
                .enabled(!pending && valid)
                .on_press(on_play)
                .child(Icon::new(IconType::Play).size(16.))
                .text(if pending {
                    t("oneclient.onboarding.account.creating")
                } else {
                    t("oneclient.onboarding.account.play_offline")
                }),
        )
        .maybe_child(error.map(|message| {
            hint_line(IconType::AlertTriangle, message, colors::danger())
        }))
        .child(divider())
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .spacing(12.)
                .child(
                    Button::new()
                        .secondary()
                        .enabled(!msa_pending)
                        .on_press(on_msa)
                        .child(Icon::new(IconType::Globe01).size(16.))
                        .text(if msa_pending {
                            t("oneclient.onboarding.account.signing_in")
                        } else {
                            t("oneclient.onboarding.account.add")
                        }),
                )
                .child(
                    label()
                        .text(t("oneclient.onboarding.account.microsoft_optional"))
                        .font_size(text_size(12.))
                        .color(colors::fg_secondary()),
                ),
        )
        .maybe_child(msa_error.map(|message| {
            hint_line(IconType::AlertTriangle, message, colors::danger())
        }))
        .into_element()
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

fn field_label(text: &str) -> impl IntoElement {
    label()
        .text(text.to_string())
        .font_size(text_size(11.))
        .font_weight(FontWeight::MEDIUM)
        .color(colors::fg_secondary())
        .into_element()
}

fn hint_line(icon: IconType, text: String, color: Color) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .child(Icon::new(icon).size(13.).color(color))
        .child(
            label()
                .text(text)
                .font_size(text_size(12.))
                .color(color),
        )
        .into_element()
}
