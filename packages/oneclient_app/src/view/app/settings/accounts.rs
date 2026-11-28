use chrono::{DateTime, Utc};
use freya::animation::{AnimNum, Function, OnFinish, use_animation};
use freya::prelude::*;
use freya::query::{MutationCapability, MutationStateData, UseMutation};
use oneclient_auth::{AccountKind, MinecraftAccount};
use uuid::Uuid;

use super::{section_header, settings_page};
use crate::Route;
use crate::components::{
    Avatar, Button, Dropdown, Icon, IconType, MicrosoftLogin, OverlayPopup, PlayerModel, TextInput,
    use_microsoft_login,
};
use crate::hooks::{
    AddOfflineAccountKeys, RefreshAccountKeys, RemoveAccountKeys, SetDefaultAccountKeys,
    SetOfflineAppearanceKeys, try_accounts, try_default_account, use_accounts,
    use_add_offline_account, use_current_account, use_refresh_account, use_remove_account,
    use_set_default_account, use_set_offline_appearance,
};
use crate::i18n::{t, tf};
use crate::layout::SearchItem;
use crate::theme::{colors, rounded, text_size};
use crate::ui::border_all_color;

/// The shader scales the model off `res.y` alone so height makes the player bigger width only needs to clear the arms
const MODEL_WIDTH_PX: f32 = 160.;
const MODEL_HEIGHT_PX: f32 = 264.;

const AVATAR_SIZE_PX: f32 = 36.;

#[derive(PartialEq)]
pub struct SettingsAccounts;

impl Component for SettingsAccounts {
    fn render(&self) -> impl IntoElement {
        let accounts_query = use_accounts();
        let default_query = use_current_account();

        let msa = use_microsoft_login();
        let add_offline = use_add_offline_account();
        let set_default = use_set_default_account();
        let remove = use_remove_account();
        let refresh = use_refresh_account();
        let set_appearance = use_set_offline_appearance();

        let mut username = use_state(String::new);
        let mut show_offline = use_state(|| false);
        let mut closing_offline = use_state(|| false);
        let editing_appearance = use_state(|| None::<Uuid>);

        use_side_effect(move || {
            if !*closing_offline.read() {
                return;
            }
            match &*add_offline.read().state() {
                MutationStateData::Settled { res: Ok(_), .. } => {
                    closing_offline.set(false);
                    show_offline.set(false);
                    username.set(String::new());
                }
                MutationStateData::Settled { res: Err(_), .. } => {
                    closing_offline.set(false);
                }
                _ => {}
            }
        });

        let accounts = try_accounts(&accounts_query).unwrap_or_default();
        let default_account = try_default_account(&default_query);
        let default_id = default_account.as_ref().map(|a| a.id);

        let offline_name = username.read().trim().to_string();
        let offline_uuid = (!offline_name.is_empty())
            .then(|| oneclient_auth::offline_uuid(&offline_name).to_string());
        // Vanilla profiles can never carry such a name so online servers will
        // reject it, but it is still useful for local testing
        let offline_warning = (!offline_name.is_empty())
            .then(|| oneclient_auth::offline_username_warning(&offline_name))
            .flatten();

        let offline_error = mutation_err_text(&add_offline);

        let on_confirm_offline = move |_| {
            let name = username.peek().trim().to_string();
            if name.is_empty() {
                return;
            }
            add_offline.mutate(AddOfflineAccountKeys { username: name });
            closing_offline.set(true);
        };

        let mut rows: Vec<Element> = accounts
            .iter()
            .map(|account| {
                AccountRow {
                    id: account.id,
                    username: account.username.clone(),
                    kind: account.kind,
                    expires: account.expires,
                    signed_out: account.needs_sign_in(),
                    is_default: Some(account.id) == default_id,
                    customizable: account.is_offline(),
                    editing_appearance,
                    set_default,
                    remove,
                    refresh,
                    sign_in: msa.clone(),
                }
                .into_element()
            })
            .collect();
        if rows.is_empty() {
            rows.push(empty_state());
        }

        let editing = editing_appearance
            .read()
            .and_then(|id| accounts.iter().find(|account| account.id == id).cloned());

        settings_page()
            .child(hero(
                default_account,
                msa.pending,
                msa.error.clone(),
                move |_| show_offline.set(true),
                {
                    let msa = msa.clone();
                    move |_| msa.start()
                },
            ))
            .child(section_header(t(
                "oneclient.settings.accounts.section.your_accounts",
            )))
            .children(rows)
            .maybe_child(show_offline.read().then(|| {
                offline_dialog(
                    username,
                    offline_uuid,
                    offline_warning,
                    offline_error,
                    on_confirm_offline,
                    show_offline,
                )
            }))
            .maybe_child(editing.map(|account| {
                AppearanceDialog {
                    account,
                    editing_appearance,
                    set_appearance,
                }
                .into_element()
            }))
            .maybe_child(msa.popup())
            .into_element()
    }
}

fn hero(
    account: Option<MinecraftAccount>,
    microsoft_pending: bool,
    error: Option<String>,
    on_open_offline: impl FnMut(Event<PressEventData>) + 'static,
    on_add_microsoft: impl FnMut(Event<PressEventData>) + 'static,
) -> impl IntoElement {
    let (name, subtitle) = match &account {
        Some(account) if account.needs_sign_in() => (
            account.username.clone(),
            t("oneclient.settings.accounts.hero.signed_out"),
        ),
        Some(account) => (account.username.clone(), kind_label(account.kind)),
        None => (
            t("oneclient.settings.accounts.hero.no_account").to_string(),
            t("oneclient.settings.accounts.hero.no_account_hint"),
        ),
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(16.)
        .padding(Gaps::new_all(16.))
        .corner_radius(rounded(12.))
        .background(colors::page_elevated())
        .child(model_frame(account.as_ref().map(|account| account.id)))
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(16.)
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(2.)
                        .child(
                            label()
                                .text(t("oneclient.settings.accounts.hero.active_account"))
                                .font_size(text_size(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_secondary()),
                        )
                        .child(
                            label()
                                .text(name)
                                .font_size(text_size(24.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .max_lines(1)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            label()
                                .text(subtitle)
                                .font_size(text_size(12.))
                                .color(colors::fg_secondary()),
                        ),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(8.)
                        .child(
                            // The two buttons overflow the ~245px beside the model at the 800px minimum so they wrap
                            rect()
                                .horizontal()
                                .width(Size::fill())
                                .content(Content::wrap_spacing(8.))
                                .spacing(8.)
                                .child(
                                    Button::new()
                                        .primary()
                                        .enabled(!microsoft_pending)
                                        .on_press(on_add_microsoft)
                                        .child(Icon::new(IconType::Globe01).size(16.))
                                        .text(if microsoft_pending {
                                            t("oneclient.settings.accounts.signing_in")
                                        } else {
                                            t("oneclient.settings.accounts.add_microsoft")
                                        }),
                                )
                                .child(
                                    Button::new()
                                        .secondary()
                                        .on_press(on_open_offline)
                                        .child(Icon::new(IconType::Plus).size(16.))
                                        .text(t("oneclient.settings.accounts.add_offline")),
                                ),
                        )
                        .map(error, |el, msg| {
                            el.child(hint_line(IconType::AlertTriangle, msg, colors::danger()))
                        }),
                ),
        )
        .into_element()
}

fn model_frame(id: Option<Uuid>) -> impl IntoElement {
    rect()
        .vertical()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .child(
            rect()
                .width(Size::px(MODEL_WIDTH_PX))
                .height(Size::px(MODEL_HEIGHT_PX))
                .center()
                .overflow(Overflow::Clip)
                .corner_radius(rounded(12.))
                .background(colors::component_bg())
                .border(border_all_color(1., colors::component_border()))
                .child(match id {
                    Some(id) => PlayerModel::new(id)
                        .yaw(-0.5)
                        .width(Size::fill())
                        .height(Size::fill())
                        .into_element(),
                    None => Icon::new(IconType::Users01)
                        .size(28.)
                        .color(colors::fg_secondary())
                        .into_element(),
                }),
        )
        .maybe_child(id.map(|_| {
            label()
                .text(t("oneclient.settings.accounts.drag_to_rotate"))
                .font_size(text_size(10.))
                .color(colors::fg_secondary())
                .into_element()
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

fn offline_dialog(
    username: State<String>,
    uuid_preview: Option<String>,
    warning: Option<String>,
    error: Option<String>,
    on_confirm: impl FnMut(Event<PressEventData>) + 'static,
    mut show_offline: State<bool>,
) -> impl IntoElement {
    OverlayPopup::new()
        .on_close(move |()| show_offline.set(false))
        .child(
            rect()
                .width(Size::window_percent(100.))
                .height(Size::window_percent(100.))
                .center()
                .child(
                    rect()
                        .vertical()
                        .width(Size::px(380.))
                        .max_width(Size::window_percent(90.))
                        .spacing(16.)
                        .padding(Gaps::new_all(20.))
                        .corner_radius(rounded(16.))
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text(t("oneclient.settings.accounts.offline.title"))
                                .font_size(text_size(18.))
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(6.)
                                .child(field_label(t(
                                    "oneclient.settings.accounts.offline.username",
                                )))
                                .child(TextInput::new(username).placeholder(t(
                                    "oneclient.settings.accounts.offline.username_placeholder",
                                )))
                                .map(warning, |el, warning| {
                                    el.child(hint_line(
                                        IconType::AlertTriangle,
                                        tf(
                                            "oneclient.settings.accounts.offline.username_warning",
                                            &[&warning],
                                        ),
                                        colors::fg_secondary(),
                                    ))
                                }),
                        )
                        .child(
                            rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(6.)
                                .child(field_label("UUID"))
                                .child(
                                    label()
                                        .text(uuid_preview.unwrap_or_else(|| "-".to_string()))
                                        .font_size(text_size(12.))
                                        .color(colors::fg_secondary()),
                                ),
                        )
                        .map(error, |el, msg| {
                            el.child(hint_line(IconType::AlertTriangle, msg, colors::danger()))
                        })
                        .child(
                            rect()
                                .horizontal()
                                .width(Size::fill())
                                .main_align(Alignment::End)
                                .spacing(8.)
                                .child(
                                    Button::new()
                                        .ghost()
                                        .on_press(move |_| show_offline.set(false))
                                        .text(t("oneclient.common.cancel")),
                                )
                                .child(
                                    Button::new()
                                        .primary()
                                        .on_press(on_confirm)
                                        .child(Icon::new(IconType::Plus).size(16.))
                                        .text(t("oneclient.settings.accounts.offline.confirm")),
                                ),
                        ),
                ),
        )
        .into_element()
}

fn field_label(text: &str) -> impl IntoElement {
    label()
        .text(text.to_string())
        .font_size(text_size(11.))
        .font_weight(FontWeight::MEDIUM)
        .color(colors::fg_secondary())
        .into_element()
}

/// Sits in whatever is left of the hero beside the model so the text must wrap
fn hint_line(icon: IconType, text: String, color: Color) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .spacing(6.)
        .child(Icon::new(icon).size(13.).color(color))
        .child(
            label()
                .text(text)
                .font_size(text_size(12.))
                .width(Size::flex(1.0))
                .color(color),
        )
        .into_element()
}

/// Skin and cape editor for offline accounts. Offline profiles have no
/// session-server textures so the launcher stores and renders these itself.
struct AppearanceDialog {
    account: MinecraftAccount,
    editing_appearance: State<Option<Uuid>>,
    set_appearance: crate::hooks::UseSetOfflineAppearance,
}

impl PartialEq for AppearanceDialog {
    fn eq(&self, other: &Self) -> bool {
        self.account.id == other.account.id
            && self.account.custom_skin() == other.account.custom_skin()
            && self.account.custom_cape() == other.account.custom_cape()
    }
}

impl Component for AppearanceDialog {
    fn render(&self) -> impl IntoElement {
        let account = self.account.clone();
        let mut editing = self.editing_appearance;
        let set_appearance = self.set_appearance;

        let skin_url = use_state(|| {
            account
                .custom_skin()
                .map(|t| t.source.clone())
                .unwrap_or_default()
        });
        let cape_url = use_state(|| {
            account
                .custom_cape()
                .map(|t| t.source.clone())
                .unwrap_or_default()
        });
        let mut closing = use_state(|| false);

        let model_options = vec![
            t("oneclient.settings.accounts.appearance.model_wide").to_string(),
            t("oneclient.settings.accounts.appearance.model_slim").to_string(),
        ];
        let mut model_idx =
            use_state(
                || match account.custom_skin().and_then(|t| t.model.as_deref()) {
                    Some(oneclient_auth::CustomTexture::SLIM) => 1,
                    _ => 0,
                },
            );

        let error = mutation_err_text(&set_appearance);

        use_side_effect(move || {
            if !*closing.read() {
                return;
            }
            match &*set_appearance.read().state() {
                MutationStateData::Settled { res: Ok(()), .. } => {
                    closing.set(false);
                    editing.set(None);
                }
                MutationStateData::Settled { res: Err(_), .. } => {
                    closing.set(false);
                }
                _ => {}
            }
        });

        let on_save = {
            let account_id = account.id;
            let mut closing = closing;
            move |_| {
                let skin = skin_url.read().trim().to_string();
                let cape = cape_url.read().trim().to_string();
                let has_skin = !skin.is_empty();
                let has_cape = !cape.is_empty();
                let model = if *model_idx.read() == 1 {
                    oneclient_auth::CustomTexture::SLIM
                } else {
                    oneclient_auth::CustomTexture::WIDE
                }
                .to_string();

                set_appearance.mutate(SetOfflineAppearanceKeys {
                    id: account_id,
                    skin_url: has_skin.then_some(skin),
                    skin_model: has_skin.then_some(model),
                    cape_url: has_cape.then_some(cape),
                });
                closing.set(true);
            }
        };

        OverlayPopup::new()
            .on_close(move |()| editing.set(None))
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
                            .spacing(16.)
                            .padding(Gaps::new_all(20.))
                            .corner_radius(rounded(16.))
                            .background(colors::page_elevated())
                            .border(border_all_color(1., colors::component_border()))
                            .child(
                                label()
                                    .text(tf(
                                        "oneclient.settings.accounts.appearance.title",
                                        &[&self.account.username],
                                    ))
                                    .font_size(text_size(18.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .max_lines(1)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                label()
                                    .text(t("oneclient.settings.accounts.appearance.note"))
                                    .font_size(text_size(12.))
                                    .color(colors::fg_secondary()),
                            )
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::fill())
                                    .spacing(6.)
                                    .child(field_label(t(
                                        "oneclient.settings.accounts.appearance.skin",
                                    )))
                                    .child(TextInput::new(skin_url).placeholder(t(
                                        "oneclient.settings.accounts.appearance.skin_placeholder",
                                    ))),
                            )
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::fill())
                                    .spacing(6.)
                                    .child(field_label(t(
                                        "oneclient.settings.accounts.appearance.model",
                                    )))
                                    .child(
                                        Dropdown::new(
                                            model_options[*model_idx.read()].clone(),
                                            model_options,
                                        )
                                        .width(Size::fill())
                                        .on_select(move |idx| model_idx.set(idx)),
                                    ),
                            )
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::fill())
                                    .spacing(6.)
                                    .child(field_label(t(
                                        "oneclient.settings.accounts.appearance.cape",
                                    )))
                                    .child(TextInput::new(cape_url).placeholder(t(
                                        "oneclient.settings.accounts.appearance.cape_placeholder",
                                    ))),
                            )
                            .map(error, |el, msg| {
                                el.child(hint_line(IconType::AlertTriangle, msg, colors::danger()))
                            })
                            .child(
                                rect()
                                    .horizontal()
                                    .width(Size::fill())
                                    .main_align(Alignment::End)
                                    .spacing(8.)
                                    .child(
                                        Button::new()
                                            .ghost()
                                            .on_press(move |_| editing.set(None))
                                            .text(t("oneclient.common.cancel")),
                                    )
                                    .child(
                                        Button::new()
                                            .primary()
                                            .on_press(on_save)
                                            .text(t("oneclient.common.save")),
                                    ),
                            ),
                    ),
            )
            .into_element()
    }
}

const REFRESH_SPIN_TIME: u64 = 800;

const REFRESHING_OPACITY: f32 = 0.7;

/// Only the identity half fades the sign-in and remove buttons stay crisp
const SIGNED_OUT_OPACITY: f32 = 0.45;

struct AccountRow {
    id: Uuid,
    username: String,
    kind: AccountKind,
    expires: DateTime<Utc>,
    signed_out: bool,
    is_default: bool,
    /// Offline accounts carry custom skins and capes the launcher renders
    /// itself so their rows open the appearance editor
    customizable: bool,
    editing_appearance: State<Option<Uuid>>,
    set_default: crate::hooks::UseSetDefaultAccount,
    remove: crate::hooks::UseRemoveAccount,
    refresh: crate::hooks::UseRefreshAccount,
    sign_in: MicrosoftLogin,
}

impl PartialEq for AccountRow {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.username == other.username
            && self.kind == other.kind
            && self.expires == other.expires
            && self.signed_out == other.signed_out
            && self.is_default == other.is_default
            && self.sign_in.pending == other.sign_in.pending
    }
}

impl Component for AccountRow {
    fn render_key(&self) -> DiffKey {
        DiffKey::from(&self.id)
    }

    fn render(&self) -> impl IntoElement {
        let id = self.id;
        let is_default = self.is_default;
        let set_default = self.set_default;
        let remove = self.remove;
        let refresh = self.refresh;

        let is_microsoft = self.kind == AccountKind::Microsoft;
        let signed_out = is_microsoft && self.signed_out;
        // A signed-out entry has no token left to call expired
        let expired = is_microsoft && !signed_out && self.expires <= Utc::now();
        let identity_opacity = if signed_out { SIGNED_OUT_OPACITY } else { 1. };
        let sign_in = self.sign_in.clone();

        let mut refreshing = use_state(|| false);
        let is_refreshing = *refreshing.read();

        let spin = use_animation(|conf| {
            conf.on_finish(OnFinish::restart());
            AnimNum::new(0., 360.)
                .time(REFRESH_SPIN_TIME)
                .function(Function::Linear)
        });

        use_side_effect_with_deps(&is_refreshing, move |&is_refreshing| {
            let mut spin = spin;
            if is_refreshing {
                spin.start();
            } else {
                spin.reset();
            }
        });

        let rotation = if is_refreshing {
            spin.get().value()
        } else {
            0.
        };

        let border_color = if signed_out {
            colors::component_border()
        } else if expired {
            colors::danger()
        } else if is_default {
            colors::brand()
        } else {
            colors::component_border()
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .cross_align(Alignment::Center)
            .content(Content::Flex)
            .spacing(12.)
            .padding(Gaps::new_all(12.))
            .corner_radius(rounded(12.))
            .background(colors::page_elevated())
            .border(border_all_color(1., border_color))
            .opacity(if is_refreshing {
                REFRESHING_OPACITY
            } else {
                1.
            })
            .a11y_role(AccessibilityRole::Button)
            .maybe(!is_default && !signed_out, |el| {
                el.on_press(move |_| set_default.mutate(SetDefaultAccountKeys { id: Some(id) }))
            })
            .child(
                rect().opacity(identity_opacity).child(
                    Avatar::new(id.to_string())
                        .width(Size::px(AVATAR_SIZE_PX))
                        .height(Size::px(AVATAR_SIZE_PX)),
                ),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(4.)
                    .opacity(identity_opacity)
                    .child(
                        rect()
                            .horizontal()
                            .cross_align(Alignment::Center)
                            .spacing(6.)
                            .child(
                                label()
                                    .text(self.username.clone())
                                    .font_size(text_size(15.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .max_lines(1)
                                    .color(colors::fg_primary()),
                            )
                            .maybe_child(is_default.then(default_badge))
                            .maybe_child(expired.then(expired_badge))
                            .maybe_child(signed_out.then(signed_out_badge)),
                    )
                    // The kind rides with the id a third badge and a full uuid do not both fit
                    .child(
                        label()
                            .text(if signed_out {
                                t("oneclient.settings.accounts.session_expired").to_string()
                            } else {
                                format!("{} · {id}", kind_label(self.kind))
                            })
                            .font_size(text_size(11.))
                            .max_lines(1)
                            .color(colors::fg_secondary()),
                    ),
            )
            .maybe_child(signed_out.then(|| {
                let pending = sign_in.pending;
                Button::new()
                    .secondary()
                    .enabled(!pending)
                    .tooltip(t("oneclient.settings.accounts.sign_in_again"))
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        sign_in.start();
                    })
                    .child(Icon::new(IconType::Globe01).size(16.))
                    .text(if pending {
                        t("oneclient.settings.accounts.signing_in")
                    } else {
                        t("oneclient.settings.accounts.sign_in")
                    })
                    .into_element()
            }))
            .maybe_child((is_microsoft && !signed_out).then(|| {
                Button::new()
                    .ghost()
                    .icon()
                    .tooltip(t("oneclient.settings.accounts.refresh"))
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        if *refreshing.peek() {
                            return;
                        }
                        refreshing.set(true);
                        spawn(async move {
                            refresh.mutate_async(RefreshAccountKeys { id }).await;
                            refreshing.set(false);
                        });
                    })
                    .child(
                        rect().rotate(rotation).child(
                            Icon::new(IconType::RefreshCw01)
                                .size(18.)
                                .color(if expired {
                                    colors::danger()
                                } else {
                                    colors::fg_secondary()
                                }),
                        ),
                    )
                    .into_element()
            }))
            .child({
                let mut edit_button = Button::new()
                    .ghost()
                    .icon()
                    .enabled(self.customizable)
                    .tooltip(if self.customizable {
                        t("oneclient.settings.accounts.edit_appearance")
                    } else {
                        t("oneclient.common.edit")
                    })
                    .child(
                        Icon::new(IconType::Pencil01)
                            .size(18.)
                            .color(colors::fg_secondary()),
                    );

                if self.customizable {
                    let mut editing = self.editing_appearance;
                    edit_button = edit_button.on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        editing.set(Some(id));
                    });
                }

                edit_button
            })
            .child(
                Button::new()
                    .ghost()
                    .icon()
                    .tooltip(t("oneclient.settings.accounts.remove"))
                    .on_press(move |e: Event<PressEventData>| {
                        e.stop_propagation();
                        remove.mutate(RemoveAccountKeys { id });
                    })
                    .child(
                        Icon::new(IconType::Trash01)
                            .size(18.)
                            .color(colors::fg_secondary()),
                    ),
            )
            .into_element()
    }
}

fn kind_label(kind: AccountKind) -> &'static str {
    match kind {
        AccountKind::Microsoft => "Microsoft",
        AccountKind::Offline => t("oneclient.settings.accounts.kind.offline"),
    }
}

fn default_badge() -> impl IntoElement {
    badge(
        Icon::new(IconType::CheckCircle)
            .size(12.)
            .color(colors::brand())
            .into_element(),
        t("oneclient.common.default").to_string(),
        colors::brand(),
        colors::brand(),
    )
}

fn expired_badge() -> impl IntoElement {
    badge(
        Icon::new(IconType::AlertTriangle)
            .size(12.)
            .color(colors::danger())
            .into_element(),
        t("oneclient.settings.accounts.badge.expired").to_string(),
        colors::danger(),
        colors::danger(),
    )
}

fn signed_out_badge() -> impl IntoElement {
    badge(
        Icon::new(IconType::AlertTriangle)
            .size(12.)
            .color(colors::fg_secondary())
            .into_element(),
        t("oneclient.settings.accounts.badge.signed_out").to_string(),
        colors::component_border(),
        colors::fg_secondary(),
    )
}

fn badge(icon: impl IntoElement, text: String, border: Color, fg: Color) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(rounded(999.))
        .border(border_all_color(1., border))
        .background(colors::component_bg())
        .child(icon)
        .child(
            label()
                .text(text)
                .font_size(text_size(10.))
                .font_weight(FontWeight::MEDIUM)
                .color(fg),
        )
        .into_element()
}

fn empty_state() -> Element {
    rect()
        .vertical()
        .width(Size::fill())
        .center()
        .padding(Gaps::new_all(32.))
        .spacing(8.)
        .corner_radius(rounded(12.))
        .background(colors::page_elevated())
        .child(
            Icon::new(IconType::Users01)
                .size(32.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text(t("oneclient.settings.accounts.empty"))
                .font_size(text_size(14.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

inventory::submit! {
    SearchItem {
        id: "nav.accounts",
        icon: IconType::Users01,
        title: "oneclient.layout.settings.tab.accounts",
        description: "oneclient.layout.search.nav.accounts.description",
        keywords: &["account", "login", "microsoft", "msa"],
        route: Route::SettingsAccounts {},
    }
}
