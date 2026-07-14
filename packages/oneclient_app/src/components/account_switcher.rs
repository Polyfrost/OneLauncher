use freya::{
    animation::{AnimNum, Ease, OnCreation, use_animation},
    prelude::*,
    router::RouterContext,
};
use oneclient_core::auth::MinecraftAccount;

use crate::{
    Route,
    components::{Avatar, Button, Icon, IconType, OverlayPopup},
    hooks::{
        SetDefaultAccountKeys, try_accounts, try_default_account, use_account_switcher_open,
        use_accounts, use_current_account, use_dispatch, use_set_default_account,
    },
    theme::colors,
};

#[derive(PartialEq)]
pub struct AccountSwitcher;

impl Component for AccountSwitcher {
    fn render(&self) -> impl IntoElement {
        let open = use_account_switcher_open();
        let dispatch = use_dispatch();

        if !open {
            return rect().into_element();
        }

        OverlayPopup::new()
            .position(Position::new_global().top(72.).right(40.))
            .on_close(move |()| dispatch.close_account_switcher())
            .child(AccountPanel)
            .into_element()
    }
}

#[derive(PartialEq)]
struct AccountPanel;

impl Component for AccountPanel {
    fn render(&self) -> impl IntoElement {
        let accounts_query = use_accounts();
        let current_query = use_current_account();

        let default_account = try_default_account(&current_query);
        let default_id = default_account.as_ref().map(|account| account.id);
        let accounts = try_accounts(&accounts_query).unwrap_or_default();

        let intro = use_animation(|conf| {
            conf.on_creation(OnCreation::Run);
            AnimNum::new(0., 1.).time(200).ease(Ease::Out)
        });
        let progress = intro.read().value();

        // active account first, then the rest
        let mut ordered: Vec<MinecraftAccount> = Vec::new();
        if let Some(account) = default_account.clone() {
            ordered.push(account);
        }
        for account in accounts {
            if Some(account.id) != default_id {
                ordered.push(account);
            }
        }

        let mut rows = rect().vertical().width(Size::fill()).spacing(4.);
        if ordered.is_empty() {
            rows = rows.child(
                label()
                    .text("No accounts yet")
                    .font_size(13.)
                    .color(colors::fg_secondary()),
            );
        } else {
            for account in ordered {
                let id = account.id;
                rows = rows.child(
                    AccountRow::new(account, Some(id) == default_id)
                        .key(id)
                        .into_element(),
                );
            }
        }

        rect()
            .vertical()
            .width(Size::px(300.))
            .padding(Gaps::new_all(12.))
            .spacing(8.)
            .opacity(progress)
            .margin(Gaps::new((1.0 - progress) * -8.0, 0., 0., 0.))
            .background(colors::page_elevated().with_a(220))
            .blur(12.)
            .corner_radius(CornerRadius::new_all(12.))
            .border(
                Border::new()
                    .fill(colors::component_border())
                    .width(1.)
                    .alignment(BorderAlignment::Inner),
            )
            .shadow(Shadow::from((
                0.,
                8.,
                32.,
                0.,
                Color::from_argb(120, 0, 0, 0),
            )))
            .child(
                label()
                    .text("Accounts")
                    .font_size(18.)
                    .font_weight(FontWeight::MEDIUM)
                    .a11y_role(AccessibilityRole::TitleBar),
            )
            .child(divider())
            .child(rows)
            .child(divider())
            .child(Footer)
    }
}

struct AccountRow {
    account: MinecraftAccount,
    active: bool,
    key: DiffKey,
}

impl PartialEq for AccountRow {
    fn eq(&self, other: &Self) -> bool {
        self.account.id == other.account.id
            && self.account.username == other.account.username
            && self.active == other.active
    }
}

impl KeyExt for AccountRow {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl AccountRow {
    fn new(account: MinecraftAccount, active: bool) -> Self {
        Self {
            account,
            active,
            key: DiffKey::None,
        }
    }
}

impl Component for AccountRow {
    fn render(&self) -> impl IntoElement {
        let id = self.account.id;
        let username = self.account.username.clone();
        let active = self.active;

        let dispatch = use_dispatch();
        let set_default = use_set_default_account();
        let mut hovered = use_state(|| false);

        let switch = move |_| {
            if !active {
                set_default.mutate(SetDefaultAccountKeys { id: Some(id) });
            }
            dispatch.close_account_switcher();
        };

        rect()
            .horizontal()
            .width(Size::fill())
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(10.)
            .padding(Gaps::new_all(8.))
            .corner_radius(CornerRadius::new_all(8.))
            .maybe(*hovered.read() && !active, |el| {
                el.background(colors::ghost_overlay_hover())
            })
            .on_pointer_enter(move |_| {
                hovered.set(true);
                if !active {
                    Cursor::set(CursorIcon::Pointer);
                }
            })
            .on_pointer_leave(move |_| {
                hovered.set(false);
                Cursor::set(CursorIcon::default());
            })
            .on_press(switch)
            .child(
                Avatar::new(id.to_string())
                    .width(Size::px(32.))
                    .height(Size::px(32.)),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .child(
                        label()
                            .text(username)
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .max_lines(1)
                            .color(colors::fg_primary()),
                    )
                    .maybe(active, |el| {
                        el.child(
                            label()
                                .text("Active")
                                .font_size(11.)
                                .color(colors::fg_secondary()),
                        )
                    }),
            )
            .maybe(active, |el| {
                el.child(
                    Icon::new(IconType::CheckCircle)
                        .size(18.)
                        .color(colors::success()),
                )
            })
    }
}

fn divider() -> impl IntoElement {
    rect()
        .width(Size::fill())
        .height(Size::px(1.))
        .background(colors::component_border())
}

#[derive(PartialEq)]
struct Footer;

impl Component for Footer {
    fn render(&self) -> impl IntoElement {
        let dispatch = use_dispatch();
        let open_accounts = move |_| {
            dispatch.close_account_switcher();
            let _ = RouterContext::get().push(Route::Accounts {});
        };

        Button::new()
            .ghost()
            .small()
            .on_press(open_accounts)
            .child(
                Icon::new(IconType::Users01)
                    .size(16.)
                    .color(colors::fg_primary()),
            )
            .text("Manage accounts")
    }
}
