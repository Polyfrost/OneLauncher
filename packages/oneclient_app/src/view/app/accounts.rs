use chrono::Utc;
use freya::prelude::*;
use freya::query::{MutationCapability, MutationStateData, UseMutation};
use freya::text_edit::Clipboard;
use oneclient_core::auth::{AccountKind, MinecraftAccount, MinecraftLogin};

use crate::components::{
    Avatar, Button, Icon, IconType, OverlayPopup, PlayerModel, ScrollArea, TextInput,
};
use crate::hooks::{
    AddOfflineAccountKeys, RefreshAccountKeys, RemoveAccountKeys, SetDefaultAccountKeys,
    accounts_have_microsoft, login_code_already_handled, try_accounts, try_default_account,
    use_accounts, use_add_offline_account, use_begin_microsoft_login, use_current_account,
    use_finish_microsoft_login, use_refresh_account, use_remove_account, use_set_default_account,
};
use crate::platform;
use crate::theme::colors;
use crate::ui::border_all_color;


#[derive(PartialEq)]
pub struct Accounts;

impl Component for Accounts {
    fn render(&self) -> impl IntoElement {
        let accounts_query = use_accounts();
        let default_query = use_current_account();

        let begin = use_begin_microsoft_login();
        let finish = use_finish_microsoft_login();
        let add_offline = use_add_offline_account();
        let set_default = use_set_default_account();
        let remove = use_remove_account();
        let refresh = use_refresh_account();

        let mut username = use_state(String::new);
        let mut pending_login = use_state(|| None::<MinecraftLogin>);
        let mut handled_code = use_state(|| None::<String>);
        let mut show_offline = use_state(|| false);
        let mut closing_offline = use_state(|| false);

        use_side_effect(move || {
            let login = match &*begin.read().state() {
                MutationStateData::Settled { res: Ok(login), .. } => Some(login.clone()),
                _ => None,
            };
            let Some(login) = login else { return };
            if login_code_already_handled(login.dedupe_key()) {
                return;
            }
            handled_code.set(Some(login.dedupe_key().to_string()));
            if let Some(url) = login.browser_url() {
                platform::open_url(url);
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
        let microsoft_pending =
            begin.read().state().is_loading() || finish.read().state().is_loading();
        let has_microsoft = accounts_have_microsoft(&accounts);

        let offline_name = username.read().trim().to_string();
        let offline_uuid = (!offline_name.is_empty())
            .then(|| oneclient_core::auth::offline_uuid(&offline_name).to_string());
        let offline_error = mutation_err_text(&add_offline);
        let microsoft_error = mutation_err_text(&finish).or_else(|| mutation_err_text(&begin));

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
                account_row(
                    account,
                    Some(account.id) == default_id,
                    set_default,
                    remove,
                    refresh,
                )
            })
            .collect();
        if rows.is_empty() {
            rows.push(empty_state());
        }

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .content(Content::Flex)
            .padding(40.)
            .spacing(24.)
            .child(render_panel(default_account))
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .overflow(Overflow::Clip)
                    .spacing(20.)
                    .child(
                        label()
                            .text("Accounts")
                            .font_size(32.)
                            .font_weight(FontWeight::BOLD)
                            .color(colors::fg_primary()),
                    )
                    .child(add_bar(
                        has_microsoft,
                        microsoft_pending,
                        microsoft_error,
                        move |_| show_offline.set(true),
                        move |_| begin.mutate(()),
                    ))
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .spacing(12.)
                            .children(rows),
                    ),
            )
            .maybe_child(show_offline.read().then(|| {
                offline_dialog(
                    username,
                    offline_uuid,
                    offline_error,
                    on_confirm_offline,
                    show_offline,
                )
            }))
            .maybe_child(
                pending_login
                    .read()
                    .clone()
                    .map(|login| microsoft_dialog(login, pending_login, handled_code)),
            )
            .into_element()
    }
}

fn render_panel(account: Option<MinecraftAccount>) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(300.))
        .height(Size::fill())
        .center()
        .spacing(16.)
        .padding(Gaps::new_all(24.))
        .corner_radius(CornerRadius::new_all(16.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        // .maybe_child(account.as_ref().map(|a| {
        //     Avatar::new(a.id.to_string())
        //         .width(Size::px(120.))
        //         .height(Size::px(120.))
        //         .into_element()
        // }))
        // .maybe_child(account.as_ref().map(|a| {
        //     label()
        //         .text(a.username.clone())
        //         .font_size(18.)
        //         .font_weight(FontWeight::SEMI_BOLD)
        //         .color(colors::fg_primary())
        //         .into_element()
        // }))
        // .maybe_child(account.is_none().then(|| {
        //     label()
        //         .text("No account selected")
        //         .font_size(14.)
        //         .color(colors::fg_secondary())
        //         .into_element()
        // }))
        // .child(
        //     label()
        //         .text("3D preview coming soon")
        //         .font_size(11.)
        //         .color(colors::fg_secondary()),
        // )
        .maybe_child(account.as_ref().map(|account| {
            PlayerModel::new(account.id)
                .yaw(-0.5)
                .width(Size::fill())
                .height(Size::fill())
                .into_element()
        }))
        .maybe_child(account.as_ref().map(|_| {
            label()
                .text("Drag to rotate the model")
                .font_size(10.)
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

fn add_bar(
    has_microsoft: bool,
    microsoft_pending: bool,
    error: Option<String>,
    on_open_offline: impl FnMut(Event<PressEventData>) + 'static,
    on_add_microsoft: impl FnMut(Event<PressEventData>) + 'static,
) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .spacing(8.)
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .cross_align(Alignment::Center)
                .main_align(Alignment::End)
                .spacing(12.)
                .child(
                    Button::new()
                        .secondary()
                        .enabled(has_microsoft)
                        .on_press(on_open_offline)
                        .child(Icon::new(IconType::Plus).size(16.))
                        .text("Add offline"),
                )
                .child(
                    Button::new()
                        .primary()
                        .enabled(!microsoft_pending)
                        .on_press(on_add_microsoft)
                        .child(Icon::new(IconType::Globe01).size(16.))
                        .text(if microsoft_pending {
                            "Signing in..."
                        } else {
                            "Add Microsoft"
                        }),
                ),
        )
        .map(error, |el, msg| {
            el.child(hint_line(IconType::AlertTriangle, msg, colors::danger()))
        })
        .maybe(!has_microsoft, |el| {
            el.child(hint_line(
                IconType::InfoCircle,
                "Add a Microsoft account before creating offline accounts.".to_string(),
                colors::fg_secondary(),
            ))
        })
        .into_element()
}

fn offline_dialog(
    username: State<String>,
    uuid_preview: Option<String>,
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
                        .corner_radius(CornerRadius::new_all(16.))
                        .background(colors::page_elevated())
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            label()
                                .text("Add offline account")
                                .font_size(18.)
                                .font_weight(FontWeight::SEMI_BOLD)
                                .color(colors::fg_primary()),
                        )
                        .child(
                            rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(6.)
                                .child(field_label("Username"))
                                .child(TextInput::new(username).placeholder("Offline username")),
                        )
                        .child(
                            rect()
                                .vertical()
                                .width(Size::fill())
                                .spacing(6.)
                                .child(field_label("UUID"))
                                .child(
                                    label()
                                        .text(uuid_preview.unwrap_or_else(|| "—".to_string()))
                                        .font_size(12.)
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
                                        .text("Cancel"),
                                )
                                .child(
                                    Button::new()
                                        .primary()
                                        .on_press(on_confirm)
                                        .child(Icon::new(IconType::Plus).size(16.))
                                        .text("Add account"),
                                ),
                        ),
                ),
        )
        .into_element()
}

fn field_label(text: &str) -> impl IntoElement {
    label()
        .text(text.to_string())
        .font_size(11.)
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
        .child(label().text(text).font_size(12.).color(color))
        .into_element()
}

fn account_row(
    account: &MinecraftAccount,
    is_default: bool,
    set_default: crate::hooks::UseSetDefaultAccount,
    remove: crate::hooks::UseRemoveAccount,
    refresh: crate::hooks::UseRefreshAccount,
) -> Element {
    let id = account.id;
    let is_microsoft = account.is_microsoft();
    let expired = is_microsoft && account.expires <= Utc::now();

    let border_color = if expired {
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
        .spacing(16.)
        .padding(Gaps::new_all(12.))
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., border_color))
        .a11y_role(AccessibilityRole::Button)
        .maybe(!is_default, |el| {
            el.on_press(move |_| set_default.mutate(SetDefaultAccountKeys { id: Some(id) }))
        })
        .child(
            Avatar::new(id.to_string())
                .width(Size::px(40.))
                .height(Size::px(40.)),
        )
        .child(
            rect()
                .vertical()
                .width(Size::flex(1.0))
                .spacing(4.)
                .child(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::Center)
                        .spacing(8.)
                        .child(
                            label()
                                .text(account.username.clone())
                                .font_size(16.)
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_primary()),
                        )
                        .child(kind_badge(account.kind))
                        .maybe_child(is_default.then(default_badge))
                        .maybe_child(expired.then(expired_badge)),
                )
                .child(
                    label()
                        .text(id.to_string())
                        .font_size(11.)
                        .color(colors::fg_secondary()),
                ),
        )
        .maybe_child(is_microsoft.then(|| {
            Button::new()
                .ghost()
                .icon()
                .on_press(move |e: Event<PressEventData>| {
                    e.stop_propagation();
                    refresh.mutate(RefreshAccountKeys { id });
                })
                .child(
                    Icon::new(IconType::RefreshCw01)
                        .size(18.)
                        .color(if expired {
                            colors::danger()
                        } else {
                            colors::fg_secondary()
                        }),
                )
                .into_element()
        }))
        .child(
            Button::new().ghost().icon().enabled(false).child(
                Icon::new(IconType::Pencil01)
                    .size(18.)
                    .color(colors::fg_secondary()),
            ),
        )
        .child(
            Button::new()
                .ghost()
                .icon()
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

fn microsoft_dialog(
    login: MinecraftLogin,
    mut pending_login: State<Option<MinecraftLogin>>,
    mut handled_code: State<Option<String>>,
) -> impl IntoElement {
    let body = match &login {
        MinecraftLogin::DeviceCode(flow) => {
            device_code_dialog_body(flow.user_code.clone(), flow.verification_uri.clone())
                .into_element()
        }
        MinecraftLogin::Browser(flow) => {
            browser_dialog_body(flow.auth_url.clone()).into_element()
        }
    };

    let mut close_pending = pending_login;
    let mut close_handled = handled_code;

    OverlayPopup::new()
        .on_close(move |()| {
            pending_login.set(None);
            handled_code.set(None);
        })
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
                        .child(body)
                        .child(waiting_row())
                        .child(
                            Button::new()
                                .ghost()
                                .on_press(move |_| {
                                    close_pending.set(None);
                                    close_handled.set(None);
                                })
                                .text("Cancel"),
                        ),
                ),
        )
        .into_element()
}

fn waiting_row() -> impl IntoElement {
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
        )
        .into_element()
}

fn browser_dialog_body(auth_url: String) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .spacing(18.)
        .child(
            label()
                .text("Your browser has opened the Microsoft sign-in page. Complete sign-in there and you'll be brought back automatically.")
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
        .spacing(18.)
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
        .into_element()
}

fn kind_badge(kind: AccountKind) -> impl IntoElement {
    let (icon, text) = match kind {
        AccountKind::Microsoft => (IconType::Globe01, "Microsoft"),
        AccountKind::Offline => (IconType::Users01, "Offline"),
    };

    badge(
        Icon::new(icon)
            .size(12.)
            .color(colors::fg_secondary())
            .into_element(),
        text.to_string(),
        colors::component_border(),
        colors::fg_secondary(),
    )
}

fn default_badge() -> impl IntoElement {
    badge(
        Icon::new(IconType::CheckCircle)
            .size(12.)
            .color(colors::brand())
            .into_element(),
        "Default".to_string(),
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
        "Expired".to_string(),
        colors::danger(),
        colors::danger(),
    )
}

fn badge(icon: impl IntoElement, text: String, border: Color, fg: Color) -> impl IntoElement {
    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(4.)
        .padding(Gaps::new_symmetric(2., 8.))
        .corner_radius(CornerRadius::new_all(999.))
        .border(border_all_color(1., border))
        .background(colors::component_bg())
        .child(icon)
        .child(
            label()
                .text(text)
                .font_size(10.)
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
        .padding(Gaps::new_all(48.))
        .spacing(8.)
        .child(
            Icon::new(IconType::Users01)
                .size(32.)
                .color(colors::fg_secondary()),
        )
        .child(
            label()
                .text("No accounts yet. Add one above.")
                .font_size(14.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}
