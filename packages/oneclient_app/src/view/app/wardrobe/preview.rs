use std::path::PathBuf;

use freya::prelude::*;
use oneclient_polyplus::api::{self, SkinModel};
use uuid::Uuid;

use super::page::{
    BACK_VIEW_TYPES, Section, Selection, Shared, VariantPick, WardrobeData, add_to_cart, cart_key,
    cart_name, collection_label, is_new, perform, perform_then, shop_thumbnail, type_label,
    variant_label,
};
use super::parts::{BACK_YAW, CapeSource, CoverThumb, FRONT_YAW, LookThumb, money, price_label};
use crate::components::{Button, Icon, IconType, ScrollArea, Segment, SegmentedControl, TextInput};
use crate::hooks::wardrobe::{WardrobeAction, use_transactions};
use crate::hooks::{query_error, query_is_loading, settled_or_loading};
use crate::theme::colors;
use crate::ui::border_all_color;

fn panel_box() -> Rect {
    rect()
        .corner_radius(CornerRadius::new_all(12.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
        .overflow(Overflow::Clip)
}

fn wardrobe_panel() -> Rect {
    rect()
        .corner_radius(CornerRadius::new_all(16.))
        .background(colors::page_elevated())
        .border(border_all_color(1., colors::component_border()))
}

fn centered(text: &str) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .main_align(Alignment::Center)
        .padding(Gaps::new_all(18.))
        .child(
            label()
                .text(text.to_string())
                .width(Size::fill())
                .text_align(TextAlign::Center)
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn centered_block(text: &str) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::px(360.))
        .main_align(Alignment::Center)
        .child(
            label()
                .text(text.to_string())
                .width(Size::fill())
                .text_align(TextAlign::Center)
                .font_size(12.)
                .color(colors::fg_secondary()),
        )
        .into_element()
}

fn status_text(status: &Option<String>) -> Option<Element> {
    status.as_ref().map(|text| {
        label()
            .text(text.clone())
            .width(Size::fill())
            .font_size(12.)
            .color(colors::fg_secondary())
            .into_element()
    })
}

fn chip_rows(chips: Vec<Element>) -> Element {
    let mut rows = rect().vertical().width(Size::fill()).spacing(6.);
    for chunk in chips.chunks(3) {
        rows = rows.child(
            rect()
                .horizontal()
                .width(Size::fill())
                .spacing(6.)
                .children(chunk.iter().cloned()),
        );
    }
    rows.into_element()
}

#[derive(PartialEq)]
pub(super) struct RightColumn {
    pub data: WardrobeData,
    pub section: Section,
    pub shared: Shared,
}

impl Component for RightColumn {
    fn render(&self) -> impl IntoElement {
        let shared = self.shared;
        let mut adding_skin = shared.adding_skin;
        let mut show_cart = shared.show_cart;
        let picked = shared.selection.read().clone();
        let data = &self.data;
        let store_enabled = data.plus_available();

        let panel = if *show_cart.read() && store_enabled {
            CartPane {
                data: data.clone(),
                shared,
                on_close: EventHandler::new(move |()| show_cart.set(false)),
            }
            .into_element()
        } else if *adding_skin.read() && store_enabled {
            AddSkinPanel {
                account: data.account,
                busy: data.busy,
                shared,
                on_close: EventHandler::new(move |()| {
                    if let Some(mut open) = adding_skin.try_write() {
                        *open = false;
                    }
                }),
            }
            .into_element()
        } else if let Some(Selection::Shop(id)) = picked
            && data.store_item(id).is_some()
        {
            StoreDetailPanel {
                data: data.clone(),
                item: id,
                shared,
            }
            .into_element()
        } else if let Some(Selection::Bundle(id)) = picked
            && data.bundle(id).is_some()
        {
            BundleDetailPanel {
                data: data.clone(),
                bundle: id,
                shared,
            }
            .into_element()
        } else {
            PreviewPanel {
                data: data.clone(),
                section: self.section,
                shared,
            }
            .into_element()
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .spacing(12.)
            .maybe_child(
                (store_enabled && data.outfits_error.is_none()).then(|| OutfitSwitcher {
                    data: data.clone(),
                    shared,
                }),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .child(panel),
            )
    }
}

#[derive(PartialEq)]
struct OutfitSwitcher {
    data: WardrobeData,
    shared: Shared,
}

impl Component for OutfitSwitcher {
    fn render(&self) -> impl IntoElement {
        let data = &self.data;
        let shared = self.shared;
        let outfits = data.outfits_in_order();
        let active_id = data.outfits.as_ref().and_then(|list| list.active_outfit_id);
        let active = outfits.iter().find(|outfit| Some(outfit.id) == active_id);
        let loaded = data.outfits.is_some();
        let can_cycle =
            !data.busy && loaded && !outfits.is_empty() && (active.is_none() || outfits.len() > 1);
        let text = match active {
            Some(outfit) => outfit.name.clone(),
            None if !loaded => "Loading outfits...".into(),
            None if outfits.is_empty() => "No saved outfits".into(),
            None => "No outfit selected".into(),
        };
        let account = data.account;

        let step = |direction: isize| {
            let index = outfits
                .iter()
                .position(|outfit| Some(outfit.id) == active_id);
            let next = match index {
                None if direction > 0 => outfits.first(),
                None => outfits.last(),
                Some(index) => {
                    let len = outfits.len() as isize;
                    outfits.get((index as isize + direction).rem_euclid(len) as usize)
                }
            };
            match next {
                Some(outfit) => shared.run(WardrobeAction::ApplyOutfit {
                    account,
                    id: outfit.id,
                    name: outfit.name.clone(),
                }),
                None => EventHandler::new(|()| {}),
            }
        };

        let previous = step(-1);
        let next = step(1);
        let create = shared.run(WardrobeAction::CreateOutfit { account });

        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(32.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(
                Button::new()
                    .secondary()
                    .icon()
                    .tooltip("Previous outfit")
                    .enabled(can_cycle)
                    .on_press(move |_| previous.call(()))
                    .child(Icon::new(IconType::ArrowLeft).size(14.)),
            )
            .child(
                rect()
                    .width(Size::flex(1.0))
                    .height(Size::fill())
                    .center()
                    .padding(Gaps::new_symmetric(0., 10.))
                    .corner_radius(CornerRadius::new_all(8.))
                    .background(colors::component_bg())
                    .border(border_all_color(1., colors::component_border()))
                    .child(
                        label()
                            .text(text)
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_size(12.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(if active.is_some() {
                                colors::fg_primary()
                            } else {
                                colors::fg_secondary()
                            }),
                    ),
            )
            .child(
                Button::new()
                    .secondary()
                    .icon()
                    .tooltip("Next outfit")
                    .enabled(can_cycle)
                    .on_press(move |_| next.call(()))
                    .child(Icon::new(IconType::ArrowRight).size(14.)),
            )
            .child(
                Button::new()
                    .secondary()
                    .icon()
                    .tooltip("New outfit from current look")
                    .enabled(!data.busy && loaded)
                    .on_press(move |_| create.call(()))
                    .child(Icon::new(IconType::Plus).size(14.)),
            )
    }
}

#[derive(PartialEq)]
struct PreviewPanel {
    data: WardrobeData,
    section: Section,
    shared: Shared,
}

impl Component for PreviewPanel {
    fn render(&self) -> impl IntoElement {
        let data = &self.data;
        let shared = self.shared;
        let picked = shared.selection.read().clone();
        let status = shared.status.read().clone();

        let owned = match &picked {
            Some(Selection::Owned(id)) => data.group(*id),
            _ => None,
        };
        let heading: Option<(String, String)> = match &picked {
            Some(Selection::MojangCape(id)) => data
                .mojang_cape(id)
                .map(|cape| (cape.alias.clone(), "Minecraft cape".to_string())),
            Some(Selection::Outfit(id)) => data
                .outfit(*id)
                .map(|outfit| (outfit.name.clone(), "Outfit".to_string())),
            Some(Selection::Skin(id)) => data.skin(*id).map(|skin| {
                (
                    skin.name.clone(),
                    if skin.model == SkinModel::Slim {
                        "Slim skin".to_string()
                    } else {
                        "Classic skin".to_string()
                    },
                )
            }),
            Some(Selection::Owned(_)) => {
                owned.map(|group| (group.name.clone(), collection_label(group.kind)))
            }
            Some(Selection::Emote(id)) => {
                data.emotes()
                    .iter()
                    .find(|emote| emote.id == *id)
                    .map(|emote| {
                        (
                            emote.name.clone(),
                            collection_label(api::CosmeticType::Emote),
                        )
                    })
            }
            _ => None,
        };

        let show_back = matches!(picked, Some(Selection::MojangCape(_)))
            || owned.is_some_and(|group| BACK_VIEW_TYPES.contains(&group.kind))
            || (picked.is_none() && self.section == Section::Cosmetics);

        let variants = owned
            .filter(|group| group.ui_variants().len() > 1)
            .map(|group| {
                let selected = data
                    .selected_variant(group)
                    .map(|variant| variant.name.clone());
                let pick = VariantPick::new(data, group);
                let chips: Vec<Element> = group
                    .ui_variants()
                    .into_iter()
                    .map(|option| {
                        let option_id = option.id;
                        let group_id = group.id;
                        let mut picks = shared.variant_picks;
                        let pick = pick.clone();
                        VariantChip {
                            text: option.name.clone(),
                            selected: selected.as_deref() == Some(option.name.as_str()),
                            on_press: EventHandler::new(move |()| {
                                picks.write().insert(group_id, option_id);
                                if let Some(keys) = pick.action(option_id) {
                                    perform(shared.action, shared.status, keys);
                                }
                            }),
                        }
                        .into_element()
                    })
                    .collect();
                rect()
                    .vertical()
                    .width(Size::fill())
                    .padding(Gaps::new_all(10.))
                    .spacing(8.)
                    .corner_radius(CornerRadius::new_all(12.))
                    .background(colors::component_bg())
                    .border(border_all_color(1., colors::component_border()))
                    .child(
                        label()
                            .text("Variants")
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    )
                    .child(chip_rows(chips))
                    .into_element()
            });

        let mut overlay = rect()
            .position(Position::new_absolute().top(0.).left(0.))
            .layer(Layer::Relative(1))
            .vertical()
            .width(Size::fill())
            .padding(Gaps::new_all(18.))
            .spacing(8.)
            .interactive(false);
        if let Some((title, subtitle)) = heading {
            overlay = overlay.child(
                rect()
                    .vertical()
                    .child(
                        label()
                            .text(title)
                            .max_lines(1)
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(subtitle)
                            .max_lines(1)
                            .font_size(12.)
                            .color(colors::fg_secondary()),
                    ),
            );
        }
        overlay = overlay.maybe_child(status_text(&status));

        let (skin, cape) = match &picked {
            Some(Selection::Skin(id)) => (
                data.skin(*id)
                    .map(|skin| (skin.url.clone(), skin.model == SkinModel::Slim)),
                data.current_cape(),
            ),
            Some(Selection::Outfit(id)) => match data.outfit(*id) {
                Some(outfit) => data.outfit_look(outfit),
                None => (None, data.current_cape()),
            },
            Some(Selection::MojangCape(id)) => (
                None,
                data.mojang_cape(id)
                    .map(|cape| CapeSource::Mojang(cape.url.clone())),
            ),
            Some(Selection::Owned(_))
                if owned.is_some_and(|group| group.kind == api::CosmeticType::Cape) =>
            {
                (
                    None,
                    owned
                        .and_then(|group| data.selected_variant(group))
                        .and_then(|variant| variant.url.clone())
                        .map(CapeSource::Plus),
                )
            }
            _ => (None, data.current_cape()),
        };

        panel_box()
            .width(Size::fill())
            .height(Size::fill())
            .center()
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(330.))
                    .child(LookThumb {
                        uuid: data.uuid.clone(),
                        skin,
                        cape,
                        yaw: if show_back { BACK_YAW } else { FRONT_YAW },
                        zoom: 1.0,
                        interactive: true,
                    }),
            )
            .child(overlay)
            .maybe_child(variants.map(|variants| {
                rect()
                    .position(Position::new_absolute().bottom(0.).left(0.))
                    .layer(Layer::Relative(1))
                    .width(Size::fill())
                    .padding(Gaps::new_all(12.))
                    .child(variants)
            }))
    }
}

#[derive(PartialEq)]
struct VariantChip {
    text: String,
    selected: bool,
    on_press: EventHandler<()>,
}

impl Component for VariantChip {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_press = self.on_press.clone();

        rect()
            .height(Size::px(28.))
            .padding(Gaps::new_symmetric(0., 10.))
            .main_align(Alignment::Center)
            .corner_radius(CornerRadius::new_all(6.))
            .background(if self.selected {
                colors::brand()
            } else {
                colors::component_bg()
            })
            .border(border_all_color(
                1.,
                if *hovered.read() && !self.selected {
                    colors::component_border_hover()
                } else {
                    colors::component_border()
                },
            ))
            .cursor(CursorIcon::Pointer)
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .child(
                label()
                    .text(self.text.clone())
                    .max_lines(1)
                    .font_size(12.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(if self.selected {
                        colors::fg_primary()
                    } else {
                        colors::fg_secondary()
                    }),
            )
    }
}

#[derive(PartialEq)]
struct CartPane {
    data: WardrobeData,
    shared: Shared,
    on_close: EventHandler<()>,
}

impl Component for CartPane {
    fn render(&self) -> impl IntoElement {
        let shared = self.shared;
        let mut cart = shared.cart;
        let items = cart.read().clone();
        let close = self.on_close.clone();
        let account = self.data.account;
        let status = shared.status.read().clone();

        let body = if items.is_empty() {
            centered("Your cart is empty.")
        } else {
            let priced = items
                .iter()
                .all(|item| item.final_price.or(item.base_price).is_some());
            let subtotal: f32 = items.iter().map(|item| item.base_price.unwrap_or(0.)).sum();
            let total: f32 = items
                .iter()
                .map(|item| item.final_price.or(item.base_price).unwrap_or(0.))
                .sum();
            let discount = if priced {
                (subtotal - total).max(0.)
            } else {
                0.
            };
            let products: Vec<String> = items.iter().map(|item| item.product_id.clone()).collect();
            let count = items.len();
            let busy = self.data.busy;
            let checkout = shared.run(WardrobeAction::Checkout { account, products });

            let rows: Vec<Element> = items
                .into_iter()
                .map(|item| {
                    let key = item.key.clone();
                    CartRow {
                        item,
                        on_remove: EventHandler::new(move |()| {
                            cart.write().retain(|entry| entry.key != key);
                        }),
                    }
                    .into_element()
                })
                .collect();

            rect()
                .vertical()
                .width(Size::fill())
                .height(Size::fill())
                .content(Content::Flex)
                .spacing(11.)
                .child(
                    ScrollArea::new()
                        .width(Size::fill())
                        .height(Size::flex(1.0))
                        .spacing(11.)
                        .children(rows),
                )
                .child(totals_row(
                    "Subtotal",
                    if priced {
                        money(subtotal)
                    } else {
                        "—".into()
                    },
                    colors::fg_primary(),
                    false,
                ))
                .maybe_child((discount > 0.).then(|| {
                    totals_row(
                        "Discounts",
                        format!("-{}", money(discount)),
                        colors::success(),
                        false,
                    )
                }))
                .child(totals_row(
                    "Total",
                    if !priced {
                        "—".into()
                    } else if total <= 0. {
                        "FREE".into()
                    } else {
                        money(total)
                    },
                    colors::fg_primary(),
                    true,
                ))
                .maybe_child(status_text(&status))
                .child(
                    Button::new()
                        .primary()
                        .width(Size::fill())
                        .enabled(!busy)
                        .on_press(move |_| checkout.call(()))
                        .child(Icon::new(IconType::LinkExternal01).size(14.))
                        .text(format!(
                            "Checkout {count} item{}",
                            if count == 1 { "" } else { "s" }
                        )),
                )
                .into_element()
        };

        wardrobe_panel()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .padding(Gaps::new_all(16.))
            .spacing(12.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .child(
                        label()
                            .text("Cart")
                            .width(Size::flex(1.0))
                            .font_size(16.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .icon()
                            .tooltip("Close")
                            .on_press(move |_| close.call(()))
                            .child(Icon::new(IconType::XClose).size(14.)),
                    ),
            )
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .child(body),
            )
    }
}

fn totals_row(text: &str, value: String, color: Color, large: bool) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .content(Content::Flex)
        .cross_align(Alignment::Center)
        .child(
            label()
                .text(text.to_string())
                .width(Size::flex(1.0))
                .font_size(if large { 18. } else { 12. })
                .color(colors::fg_primary()),
        )
        .child(
            label()
                .text(value)
                .font_size(if large { 22. } else { 12. })
                .font_weight(if large {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .color(color),
        )
        .into_element()
}

#[derive(PartialEq)]
struct CartRow {
    item: super::page::CartEntry,
    on_remove: EventHandler<()>,
}

impl Component for CartRow {
    fn render(&self) -> impl IntoElement {
        let item = &self.item;
        let remove = self.on_remove.clone();

        panel_box()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(81.))
            .content(Content::Flex)
            .padding(Gaps::new_symmetric(11., 13.))
            .cross_align(Alignment::Center)
            .spacing(13.)
            .child(
                rect()
                    .width(Size::px(58.))
                    .height(Size::px(58.))
                    .corner_radius(CornerRadius::new_all(5.))
                    .background(colors::page())
                    .overflow(Overflow::Clip)
                    .child(CoverThumb {
                        url: item.cover_asset_id.map(api::asset_url),
                    }),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(item.name.clone())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .width(Size::fill())
                            .font_size(14.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .maybe_child(
                        item.description
                            .clone()
                            .filter(|text| !text.trim().is_empty())
                            .map(|text| {
                                label()
                                    .text(text)
                                    .max_lines(2)
                                    .text_overflow(TextOverflow::Ellipsis)
                                    .width(Size::fill())
                                    .font_size(12.)
                                    .color(colors::fg_secondary())
                            }),
                    ),
            )
            .child(
                rect()
                    .vertical()
                    .cross_align(Alignment::End)
                    .spacing(2.)
                    .child(price_label(
                        item.base_price,
                        item.final_price,
                        item.discounted(),
                    ))
                    .maybe_child(item.discounted().then(|| {
                        label()
                            .text(format!("SAVE {}%", item.discount_percent.unwrap_or(0)))
                            .font_size(12.)
                            .color(colors::success())
                    }))
                    .child(
                        Button::new()
                            .ghost()
                            .icon()
                            .tooltip("Remove")
                            .on_press(move |_| remove.call(()))
                            .child(Icon::new(IconType::Trash01).size(14.)),
                    ),
            )
    }
}

#[derive(PartialEq)]
struct StoreDetailPanel {
    data: WardrobeData,
    item: i32,
    shared: Shared,
}

impl Component for StoreDetailPanel {
    fn render(&self) -> impl IntoElement {
        let data = &self.data;
        let shared = self.shared;
        let status = shared.status.read().clone();
        let Some(item) = data.store_item(self.item).cloned() else {
            return panel_box()
                .width(Size::fill())
                .height(Size::fill())
                .child(centered("Select a cosmetic to preview it."))
                .into_element();
        };
        let variant = data.shop_variant(&item);
        let owned = data.owned_ids().contains(&variant.id);
        let in_cart = data.cart_keys.contains(&cart_key(variant.id));
        let variants = item.ui_variants();
        let mut picks = shared.store_picks;

        let badge = if owned {
            Some(("OWNED", colors::success()))
        } else if is_new(&item.created_at) {
            Some(("NEW", colors::brand()))
        } else {
            None
        };

        let variant_rows = (variants.len() > 1).then(|| {
            let item_id = item.id;
            let chips: Vec<Element> = variants
                .iter()
                .map(|option| {
                    let option_id = option.id;
                    VariantChip {
                        text: variant_label(option, &item),
                        selected: variant_label(option, &item) == variant_label(&variant, &item),
                        on_press: EventHandler::new(move |()| {
                            picks.write().insert(item_id, option_id);
                        }),
                    }
                    .into_element()
                })
                .collect();
            chip_rows(chips)
        });

        let tags: Vec<String> = item
            .tags
            .custom
            .iter()
            .chain(item.tags.colors.iter())
            .take(4)
            .map(|tag| {
                let mut chars = tag.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect();

        let add = {
            let item = item.clone();
            let variant = variant.clone();
            EventHandler::new(move |()| {
                if !in_cart {
                    add_to_cart(owned, shared.cart, shared.status, &item, &variant);
                }
            })
        };
        let buy = shared.run(WardrobeAction::BuyNow {
            account: data.account,
            cosmetic: variant.id,
            name: cart_name(&item, &variant),
        });

        let info = rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(
                label()
                    .text(type_label(item.kind))
                    .font_size(12.)
                    .color(colors::fg_secondary()),
            )
            .child(
                label()
                    .text(item.name.clone())
                    .width(Size::fill())
                    .font_size(16.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            )
            .child(price_label(
                item.base_price,
                item.final_price(),
                item.discounted(),
            ))
            .maybe_child(variant_rows)
            .maybe_child((!tags.is_empty()).then(|| {
                rect()
                    .horizontal()
                    .spacing(6.)
                    .children(tags.into_iter().map(|tag| {
                        rect()
                            .padding(Gaps::new_symmetric(3., 8.))
                            .corner_radius(CornerRadius::new_all(5.))
                            .background(colors::component_bg())
                            .border(border_all_color(1., colors::component_border()))
                            .child(label().text(tag).font_size(11.).color(colors::fg_primary()))
                            .into_element()
                    }))
            }))
            .maybe_child(
                item.description
                    .clone()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| {
                        label()
                            .text(text)
                            .width(Size::fill())
                            .font_size(12.)
                            .color(colors::fg_secondary())
                    }),
            );

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .spacing(14.)
            .child(
                panel_box()
                    .width(Size::fill())
                    .height(Size::px(330.))
                    .child(shop_thumbnail(data, &item, &variant, true))
                    .maybe_child(badge.map(|(text, color)| {
                        rect()
                            .position(Position::new_absolute().top(12.).left(12.))
                            .layer(Layer::Relative(1))
                            .padding(Gaps::new_symmetric(3., 8.))
                            .corner_radius(CornerRadius::new_all(5.))
                            .background(color)
                            .child(
                                label()
                                    .text(text)
                                    .font_size(11.)
                                    .font_weight(FontWeight::MEDIUM)
                                    .color(colors::fg_primary()),
                            )
                    })),
            )
            .child(
                panel_box()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::flex(1.0))
                    .content(Content::Flex)
                    .padding(Gaps::new_all(18.))
                    .spacing(10.)
                    .child(
                        ScrollArea::new()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .child(info),
                    )
                    .maybe_child(status_text(&status))
                    .child(if owned {
                        label()
                            .text("You already own this cosmetic.")
                            .font_size(12.)
                            .color(colors::fg_secondary())
                            .into_element()
                    } else {
                        rect()
                            .horizontal()
                            .spacing(8.)
                            .child(
                                Button::new()
                                    .secondary()
                                    .enabled(!in_cart && !data.busy && data.plus.is_some())
                                    .on_press(move |_| add.call(()))
                                    .maybe_child(
                                        (!in_cart).then(|| Icon::new(IconType::Plus).size(14.)),
                                    )
                                    .text(if in_cart { "In cart" } else { "Add to cart" }),
                            )
                            .child(
                                Button::new()
                                    .primary()
                                    .enabled(!data.busy && data.plus.is_some())
                                    .on_press(move |_| buy.call(()))
                                    .child(Icon::new(IconType::LinkExternal01).size(14.))
                                    .text("Buy now"),
                            )
                            .into_element()
                    }),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
struct BundleDetailPanel {
    data: WardrobeData,
    bundle: i32,
    shared: Shared,
}

impl Component for BundleDetailPanel {
    fn render(&self) -> impl IntoElement {
        let shared = self.shared;
        let status = shared.status.read().clone();
        let Some(bundle) = self.data.bundle(self.bundle).cloned() else {
            return panel_box()
                .width(Size::fill())
                .height(Size::fill())
                .into_element();
        };
        let mut status_state = shared.status;
        let buy = match bundle.store_product_id.clone() {
            Some(product) => shared.run(WardrobeAction::Checkout {
                account: self.data.account,
                products: vec![product],
            }),
            None => {
                let message = format!("{} isn't purchasable yet.", bundle.name);
                EventHandler::new(move |()| status_state.set(Some(message.clone())))
            }
        };

        wardrobe_panel()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .content(Content::Flex)
            .padding(Gaps::new_all(16.))
            .spacing(10.)
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(200.))
                    .corner_radius(CornerRadius::new_all(5.))
                    .background(colors::page())
                    .overflow(Overflow::Clip)
                    .child(CoverThumb {
                        url: bundle.asset_id.map(api::asset_url),
                    }),
            )
            .child(
                label()
                    .text("Bundle")
                    .font_size(12.)
                    .color(colors::fg_secondary()),
            )
            .child(
                label()
                    .text(bundle.name.clone())
                    .width(Size::fill())
                    .font_size(16.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(colors::fg_primary()),
            )
            .child(price_label(
                bundle.base_price,
                bundle.final_price(),
                bundle.discounted(),
            ))
            .maybe_child(
                bundle
                    .description
                    .clone()
                    .filter(|text| !text.trim().is_empty())
                    .map(|text| {
                        label()
                            .text(text)
                            .width(Size::fill())
                            .font_size(12.)
                            .color(colors::fg_secondary())
                    }),
            )
            .child(rect().width(Size::fill()).height(Size::flex(1.0)))
            .maybe_child(status_text(&status))
            .child(
                Button::new()
                    .primary()
                    .width(Size::fill())
                    .enabled(!self.data.busy)
                    .on_press(move |_| buy.call(()))
                    .child(Icon::new(IconType::LinkExternal01).size(14.))
                    .text("Buy now"),
            )
            .into_element()
    }
}

#[derive(PartialEq)]
pub(super) struct HistoryList {
    pub account: Uuid,
}

impl Component for HistoryList {
    fn render(&self) -> impl IntoElement {
        let query = use_transactions(self.account);
        let transactions = settled_or_loading(&query);
        let error = query_error(&query);

        let body = match (&transactions, &error) {
            (None, Some(error)) => centered_block(error),
            (None, None) if query_is_loading(&query) => centered_block("Loading transactions..."),
            (Some(list), _) if list.is_empty() => centered_block("No transactions yet."),
            (Some(list), _) => rect()
                .vertical()
                .width(Size::fill())
                .spacing(9.)
                .children(list.iter().map(|tx| {
                    let (status, color) = status_style(&tx.status);
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .padding(Gaps::new_symmetric(10., 13.))
                        .spacing(13.)
                        .corner_radius(CornerRadius::new_all(8.))
                        .border(border_all_color(1., colors::component_border()))
                        .child(
                            rect()
                                .vertical()
                                .width(Size::flex(1.0))
                                .child(
                                    label()
                                        .text(format!("Order #{}", tx.id))
                                        .font_size(14.)
                                        .font_weight(FontWeight::MEDIUM)
                                        .color(colors::fg_primary()),
                                )
                                .child(
                                    label()
                                        .text(provider_name(&tx.provider))
                                        .font_size(12.)
                                        .color(colors::fg_secondary()),
                                ),
                        )
                        .maybe_child(tx.amount_minor.map(|minor| {
                            let amount = minor as f32 / 100.;
                            let text = match tx.currency.as_deref() {
                                Some(currency) if !currency.eq_ignore_ascii_case("usd") => {
                                    format!("{amount:.2} {}", currency.to_uppercase())
                                }
                                _ => money(amount),
                            };
                            label()
                                .text(text)
                                .font_size(14.)
                                .font_weight(FontWeight::MEDIUM)
                                .color(colors::fg_primary())
                        }))
                        .child(
                            label()
                                .text(status)
                                .font_size(12.)
                                .font_weight(FontWeight::MEDIUM)
                                .color(color),
                        )
                        .into_element()
                }))
                .into_element(),
            _ => centered_block("Loading transactions..."),
        };

        panel_box()
            .width(Size::fill())
            .min_height(Size::px(400.))
            .padding(Gaps::new_all(18.))
            .child(body)
    }
}

fn provider_name(provider: &str) -> String {
    match provider {
        "stripe" => "Stripe".into(),
        "paynow" => "PayNow".into(),
        "ingame" => "In-game".into(),
        "admin_grant" => "Admin grant".into(),
        _ => "Unknown".into(),
    }
}

fn status_style(status: &str) -> (String, Color) {
    let muted = colors::fg_secondary();
    match status {
        "completed" => ("Completed".into(), colors::success()),
        "pending" => ("Pending".into(), colors::code_warn()),
        "failed" => ("Failed".into(), colors::danger()),
        "refunded" => ("Refunded".into(), muted),
        "partially_refunded" => ("Partially refunded".into(), muted),
        "chargeback" => ("Chargeback".into(), muted),
        _ => ("Unknown".into(), muted),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SkinSource {
    Username,
    Url,
    File,
}

#[derive(PartialEq)]
struct AddSkinPanel {
    account: Uuid,
    busy: bool,
    shared: Shared,
    on_close: EventHandler<()>,
}

impl Component for AddSkinPanel {
    fn render(&self) -> impl IntoElement {
        let shared = self.shared;
        let source = use_state(|| SkinSource::Username);
        let username = use_state(String::new);
        let url = use_state(String::new);
        let name = use_state(String::new);
        let mut file = use_state(|| None::<PathBuf>);
        let slim = use_state(|| false);
        let mut message = use_state(|| None::<String>);

        let account = self.account;
        let close = self.on_close.clone();
        let done = self.on_close.clone();
        let picked = *source.read();

        use_side_effect_with_deps(&picked, move |_| message.set(None));

        let submit = EventHandler::new(move |()| {
            let custom_name: String = name.read().trim().chars().take(64).collect();
            let custom_name = custom_name.trim().to_string();
            let custom_name = (!custom_name.is_empty()).then_some(custom_name);
            let keys = match *source.read() {
                SkinSource::Username => {
                    let username = username.read().trim().to_string();
                    if username.is_empty() {
                        message.set(Some("Enter a Minecraft username.".into()));
                        return;
                    }
                    WardrobeAction::AddSkinUsername { account, username }
                }
                SkinSource::Url => {
                    let url = url.read().trim().to_string();
                    if !url.starts_with("http://") && !url.starts_with("https://") {
                        message.set(Some("Enter a link to a PNG skin.".into()));
                        return;
                    }
                    WardrobeAction::AddSkinUrl {
                        account,
                        url,
                        slim: *slim.read(),
                        name: custom_name,
                    }
                }
                SkinSource::File => {
                    let Some(path) = file.read().clone() else {
                        message.set(Some("Choose a PNG file.".into()));
                        return;
                    };
                    WardrobeAction::AddSkinFile {
                        account,
                        path,
                        slim: *slim.read(),
                        name: custom_name,
                    }
                }
            };
            message.set(None);
            let done = done.clone();
            perform_then(shared.action, shared.status, keys, move || {
                if message.try_peek().is_some() {
                    done.call(());
                }
            });
        });

        let mut body = rect().vertical().width(Size::fill()).spacing(12.);

        body = match picked {
            SkinSource::Username => body.child(
                TextInput::new(username)
                    .placeholder("Minecraft username")
                    .compact()
                    .width(Size::fill()),
            ),
            SkinSource::Url => body.child(
                TextInput::new(url)
                    .placeholder("Link to a PNG skin")
                    .compact()
                    .width(Size::fill()),
            ),
            SkinSource::File => {
                let chosen = file
                    .read()
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .map(|name| name.to_string_lossy().to_string());
                body.child(
                    rect()
                        .horizontal()
                        .width(Size::fill())
                        .content(Content::Flex)
                        .cross_align(Alignment::Center)
                        .spacing(10.)
                        .child(
                            Button::new()
                                .secondary()
                                .on_press(move |_| {
                                    spawn(async move {
                                        if let Some(handle) = rfd::AsyncFileDialog::new()
                                            .set_title("Choose a skin")
                                            .add_filter("PNG image", &["png"])
                                            .pick_file()
                                            .await
                                        {
                                            file.set(Some(handle.path().to_path_buf()));
                                        }
                                    });
                                })
                                .child(Icon::new(IconType::Folder).size(14.))
                                .text("Choose file"),
                        )
                        .child(
                            label()
                                .text(chosen.unwrap_or_else(|| "No file chosen".into()))
                                .width(Size::flex(1.0))
                                .max_lines(1)
                                .text_overflow(TextOverflow::Ellipsis)
                                .font_size(12.)
                                .color(colors::fg_secondary()),
                        ),
                )
            }
        };

        if picked != SkinSource::Username {
            body = body
                .child(
                    TextInput::new(name)
                        .placeholder("Name (optional)")
                        .compact()
                        .width(Size::fill()),
                )
                .child(
                    SegmentedControl::new(slim)
                        .segment(Segment::new(false).label("Classic"))
                        .segment(Segment::new(true).label("Slim")),
                );
        }

        let note = match picked {
            SkinSource::Username => "Saves that player's skin to your library.",
            _ => "Adding a skin uploads it to Minecraft and puts it on.",
        };

        wardrobe_panel()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new_all(16.))
            .spacing(12.)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .content(Content::Flex)
                    .cross_align(Alignment::Center)
                    .child(
                        label()
                            .text("Add skin")
                            .width(Size::flex(1.0))
                            .font_size(16.)
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        Button::new()
                            .secondary()
                            .icon()
                            .tooltip("Close")
                            .on_press(move |_| close.call(()))
                            .child(Icon::new(IconType::XClose).size(14.)),
                    ),
            )
            .child(
                SegmentedControl::new(source)
                    .segment(
                        Segment::new(SkinSource::Username)
                            .icon(IconType::Users01)
                            .label("Username"),
                    )
                    .segment(
                        Segment::new(SkinSource::Url)
                            .icon(IconType::Link03)
                            .label("URL"),
                    )
                    .segment(
                        Segment::new(SkinSource::File)
                            .icon(IconType::Folder)
                            .label("File"),
                    ),
            )
            .child(body)
            .child(
                label()
                    .text(message.read().clone().unwrap_or_else(|| note.to_string()))
                    .width(Size::fill())
                    .font_size(12.)
                    .color(colors::fg_secondary()),
            )
            .maybe_child(status_text(&shared.status.read().clone()))
            .child(
                Button::new()
                    .primary()
                    .width(Size::fill())
                    .enabled(!self.busy)
                    .on_press(move |_| submit.call(()))
                    .child(Icon::new(IconType::Plus).size(14.))
                    .text("Add skin"),
            )
    }
}
