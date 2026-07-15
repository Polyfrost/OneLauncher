use freya::prelude::*;

use crate::components::{Button, ButtonVariant, Icon, IconType, PlayerModel};
use crate::hooks::{try_default_account, use_current_account};
use crate::theme::colors;
use crate::ui::border_all_color;

const ANIMATIONS: &[&str] = &["Idle", "Walking", "Running", "Crouch", "Flying"];

#[derive(PartialEq)]
pub struct AccountSkins;

impl Component for AccountSkins {
    fn render(&self) -> impl IntoElement {
        let account_uuid = try_default_account(&use_current_account()).map(|a| a.id.to_string());

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .overflow(Overflow::Clip)
            .padding(40.)
            .spacing(24.)
            .child(preview_panel(account_uuid))
            .child(side_panel())
    }
}

fn preview_panel(account_uuid: Option<String>) -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::px(320.))
        .height(Size::fill())
        .spacing(16.)
        .child(
            rect()
                .center()
                .width(Size::fill())
                .height(Size::flex(1.0))
                .corner_radius(CornerRadius::new_all(16.))
                .background(colors::page_elevated())
                .border(border_all_color(1., colors::component_border()))
                .child(match account_uuid {
                    Some(uuid) => PlayerModel::new(uuid)
                        .width(Size::fill())
                        .height(Size::fill())
                        .into_element(),
                    None => Icon::new(IconType::Users01)
                        .size(64.)
                        .color(colors::fg_secondary())
                        .into_element(),
                }),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .main_align(Alignment::Center)
                .spacing(8.)
                .children(
                    ANIMATIONS
                        .iter()
                        .enumerate()
                        .map(|(i, name)| anim_chip(name, i == 0).into_element()),
                ),
        )
        .into_element()
}

fn anim_chip(name: &'static str, active: bool) -> impl IntoElement {
    rect()
        .center()
        .padding(Gaps::new_symmetric(6., 12.))
        .corner_radius(CornerRadius::new_all(8.))
        .background(if active {
            colors::brand()
        } else {
            colors::component_bg()
        })
        .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
        .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
        .child(
            label()
                .text(name)
                .font_size(12.)
                .color(colors::fg_primary()),
        )
        .into_element()
}

fn side_panel() -> impl IntoElement {
    rect()
        .vertical()
        .width(Size::flex(1.0))
        .height(Size::fill())
        .overflow(Overflow::Clip)
        .spacing(20.)
        .child(
            label()
                .text("Skins")
                .font_size(32.)
                .font_weight(FontWeight::BOLD)
                .color(colors::fg_primary()),
        )
        .child(
            rect()
                .horizontal()
                .spacing(12.)
                .child(action_button("Import Skin", IconType::Plus, true))
                .child(action_button("Download", IconType::Download01, false))
                .child(action_button("Remove", IconType::Trash01, false)),
        )
        .child(section_label("SKIN HISTORY"))
        .child(skin_grid(8))
        .child(section_label("CAPES"))
        .child(skin_grid(4))
        .into_element()
}

fn action_button(text: &'static str, icon: IconType, primary: bool) -> impl IntoElement {
    Button::new()
        .variant(if primary {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        })
        .child(Icon::new(icon).size(16.).color(colors::fg_primary()))
        .text(text)
        .into_element()
}

fn section_label(text: &'static str) -> impl IntoElement {
    label()
        .text(text)
        .font_size(11.)
        .font_weight(FontWeight::SEMI_BOLD)
        .color(colors::fg_secondary())
        .into_element()
}

fn skin_grid(count: usize) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .spacing(12.)
        .children((0..count).map(|i| {
            rect()
                .center()
                .width(Size::px(64.))
                .height(Size::px(80.))
                .corner_radius(CornerRadius::new_all(8.))
                .background(colors::page_elevated())
                .border(border_all_color(
                    if i == 0 { 2. } else { 1. },
                    if i == 0 {
                        colors::brand()
                    } else {
                        colors::component_border()
                    },
                ))
                .on_pointer_enter(|_| Cursor::set(CursorIcon::Pointer))
                .on_pointer_leave(|_| Cursor::set(CursorIcon::default()))
                .child(
                    Icon::new(IconType::Users01)
                        .size(24.)
                        .color(colors::fg_secondary()),
                )
                .into_element()
        }))
        .into_element()
}
