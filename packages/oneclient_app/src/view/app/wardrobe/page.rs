use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::time::Duration;

use freya::animation::*;
use freya::prelude::*;
use freya::query::{MutationStateData, QueryCapability, UseQuery};
use oneclient_mc::{MojangCape, MojangFullPlayerProfile};
use oneclient_polyplus::api::{
    self, BodySlot, BundleInfo, CosmeticGroup, CosmeticType, CosmeticVariant, EmoteInfo,
    OutfitInfo, OutfitList, PlayerCosmetics, SkinInfo, SkinList, SkinModel, StoreItem,
    StoreVariant, texture_hash,
};
use uuid::Uuid;

use super::parts::{
    AnchorChip, BACK_YAW, Badge, BadgeStyle, CAPE_ZOOM, CARD_CHROME, CARD_WIDTH, CapeSource,
    CoverThumb, FRONT_YAW, GRID_GAP, InfoHeading, LookThumb, MenuAction, Rename, SectionChip,
    Swatch, WardrobeCard, detail_text, divider, library_intro, notice, price_label, store_label,
};
use super::preview::{HistoryList, RightColumn};
use crate::components::{
    Button, Dropdown, Icon, IconType, ScrollArea, ScrollAreaCtx, Spinner, TextInput,
    centered_spinner,
};
use crate::hooks::wardrobe::{
    MojangLookQuery, OutfitsQuery, PlusCosmeticsQuery, SkinsQuery, StoreData, StoreQuery,
    UseWardrobeAction, WardrobeAction, active_cape, active_skin, invalidate_wardrobe_queries,
    plus_message, use_mojang_look, use_outfits, use_plus_cosmetics, use_skin_library, use_store,
    use_store_suggestions, use_wardrobe_action,
};
use crate::hooks::{
    mutation_is_running, query_error, query_is_loading, settled_or_loading, try_default_account,
    use_current_account, use_debounced, use_overlay_claim_when,
};
use crate::i18n::{t, tf};
use crate::theme::{colors, rounded, text_size};
use crate::ui::{border_all_color, centered_note};
use crate::view::app::cluster::confirm_dialog;

const RIGHT_COLUMN_MIN_WIDTH: f32 = 320.;

pub(super) fn categories() -> [(CosmeticType, &'static str); 10] {
    [
        (CosmeticType::Cape, t("oneclient.wardrobe.category.cape")),
        (
            CosmeticType::Backpack,
            t("oneclient.wardrobe.category.back"),
        ),
        (
            CosmeticType::Glasses,
            t("oneclient.wardrobe.category.glasses"),
        ),
        (CosmeticType::Wings, t("oneclient.wardrobe.category.wings")),
        (CosmeticType::Glove, t("oneclient.wardrobe.category.glove")),
        (CosmeticType::Hat, t("oneclient.wardrobe.category.hat")),
        (CosmeticType::Aura, t("oneclient.wardrobe.category.aura")),
        (CosmeticType::Boots, t("oneclient.wardrobe.category.boots")),
        (
            CosmeticType::Shoulder,
            t("oneclient.wardrobe.category.shoulder"),
        ),
        (CosmeticType::Pet, t("oneclient.wardrobe.category.pet")),
    ]
}

pub(super) const BACK_VIEW_TYPES: [CosmeticType; 3] = [
    CosmeticType::Cape,
    CosmeticType::Wings,
    CosmeticType::Backpack,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Section {
    Cosmetics,
    Outfits,
    Skins,
}

fn section_tabs() -> [(Section, &'static str, IconType); 3] {
    [
        (
            Section::Cosmetics,
            t("oneclient.wardrobe.section.cosmetics"),
            IconType::Colors,
        ),
        (
            Section::Outfits,
            t("oneclient.wardrobe.section.outfits"),
            IconType::Users01,
        ),
        (
            Section::Skins,
            t("oneclient.wardrobe.section.skins"),
            IconType::Brush01,
        ),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Sort {
    Popular,
    Newest,
    PriceLow,
    PriceHigh,
    Name,
}

impl Sort {
    fn label(self) -> &'static str {
        match self {
            Self::Popular => t("oneclient.wardrobe.sort.popular"),
            Self::Newest => t("oneclient.wardrobe.sort.newest"),
            Self::PriceLow => t("oneclient.wardrobe.sort.price_low"),
            Self::PriceHigh => t("oneclient.wardrobe.sort.price_high"),
            Self::Name => t("oneclient.wardrobe.sort.name"),
        }
    }

    const ALL: [Sort; 5] = [
        Self::Popular,
        Self::Newest,
        Self::PriceLow,
        Self::PriceHigh,
        Self::Name,
    ];
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Selection {
    MojangCape(String),
    Owned(i32),
    Shop(i32),
    Bundle(i32),
    Emote(i32),
    Outfit(i32),
    Skin(i32),
}

#[derive(Clone, PartialEq, Debug)]
pub(super) struct CartEntry {
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub base_price: Option<f32>,
    pub final_price: Option<f32>,
    pub discount_percent: Option<i32>,
    pub cosmetic: i32,
    pub product_id: String,
    pub cover_asset_id: Option<i32>,
}

impl CartEntry {
    pub fn discounted(&self) -> bool {
        self.discount_percent.unwrap_or(0) > 0 && self.final_price.unwrap_or(0.) > 0.
    }
}

pub(super) fn cart_key(variant: i32) -> String {
    format!("cosmetic-{variant}")
}

#[derive(Clone, PartialEq)]
pub(super) struct WardrobeData {
    pub account: Uuid,
    pub uuid: String,
    pub look: Option<MojangFullPlayerProfile>,
    pub look_error: Option<String>,
    pub plus: Option<PlayerCosmetics>,
    pub plus_error: Option<String>,
    pub outfits: Option<OutfitList>,
    pub outfits_error: Option<String>,
    pub skins: Option<SkinList>,
    pub skins_error: Option<String>,
    pub store: Option<StoreData>,
    pub store_error: Option<String>,
    pub busy: bool,
    pub slim: bool,
    pub grid_width: f32,
    pub sort: Sort,
    pub query: String,
    pub variant_picks: HashMap<i32, Pick>,
    pub store_picks: HashMap<i32, i32>,
    pub cart_keys: Vec<String>,
    pub owned: HashSet<i32>,
}

fn owned_set(plus: Option<&PlayerCosmetics>) -> HashSet<i32> {
    let mut ids: HashSet<i32> = plus
        .iter()
        .flat_map(|plus| plus.cosmetics.iter())
        .flat_map(|group| group.variants.iter().map(|variant| variant.id))
        .collect();
    ids.extend(
        plus.iter()
            .flat_map(|plus| plus.emotes.iter())
            .map(|emote| emote.id),
    );
    ids
}

impl WardrobeData {
    pub fn plus_available(&self) -> bool {
        self.plus.is_some() || self.plus_error.is_none()
    }

    pub fn matches(&self, name: &str) -> bool {
        let needle = self.query.trim();
        needle.is_empty() || name.to_lowercase().contains(&needle.to_lowercase())
    }

    pub fn groups(&self, kind: CosmeticType) -> Vec<&CosmeticGroup> {
        self.plus
            .iter()
            .flat_map(|plus| plus.cosmetics.iter())
            .filter(|group| group.kind == kind && self.matches(&group.name))
            .collect()
    }

    pub fn emotes(&self) -> &[EmoteInfo] {
        self.plus
            .as_ref()
            .map_or(&[], |plus| plus.emotes.as_slice())
    }

    pub fn owned_ids(&self) -> &HashSet<i32> {
        &self.owned
    }

    pub fn group(&self, id: i32) -> Option<&CosmeticGroup> {
        self.plus
            .iter()
            .flat_map(|plus| plus.cosmetics.iter())
            .find(|group| group.id == id)
    }

    pub fn variant(&self, id: i32) -> Option<(&CosmeticGroup, &CosmeticVariant)> {
        self.plus
            .iter()
            .flat_map(|plus| plus.cosmetics.iter())
            .find_map(|group| {
                group
                    .variants
                    .iter()
                    .find(|variant| variant.id == id)
                    .map(|variant| (group, variant))
            })
    }

    /// The slots this group is worn in, with the variant in each
    pub fn worn(&self, group: &CosmeticGroup) -> Pick {
        let mut worn: Pick = self
            .plus
            .iter()
            .flat_map(|plus| plus.equipped.iter())
            .filter(|(_, id)| group.variants.iter().any(|variant| variant.id == **id))
            .map(|(slot, id)| (*slot, *id))
            .collect();
        worn.sort();
        worn
    }

    /// What the preview shows and Equip applies: the user's pick, else what is worn, else the first variant
    pub fn pick(&self, group: &CosmeticGroup) -> Pick {
        if let Some(pick) = self.variant_picks.get(&group.id) {
            return pick.clone();
        }
        let worn = self.worn(group);
        if !worn.is_empty() {
            return worn;
        }
        group
            .variants
            .first()
            .map(|variant| variant_pick(group, variant))
            .unwrap_or_default()
    }

    /// The pick with each variant swapped for the one matching the skin's arm model
    pub fn resolved_pick(&self, group: &CosmeticGroup) -> Pick {
        let mut pick: Pick = self
            .pick(group)
            .into_iter()
            .map(|(slot, id)| (slot, group.variant_for_skin(id, self.slim)))
            .collect();
        pick.sort();
        pick
    }

    pub fn pick_worn(&self, group: &CosmeticGroup) -> bool {
        let worn = self.worn(group);
        !worn.is_empty() && worn == self.resolved_pick(group)
    }

    pub fn selected_variant<'a>(&self, group: &'a CosmeticGroup) -> Option<&'a CosmeticVariant> {
        self.pick(group)
            .first()
            .and_then(|(_, id)| group.variants.iter().find(|variant| variant.id == *id))
            .or_else(|| group.variants.first())
    }

    pub fn shop_variant(&self, item: &StoreItem) -> StoreVariant {
        let variants = item.ui_variants();
        let owned = self.owned_ids();
        self.store_picks
            .get(&item.id)
            .and_then(|id| variants.iter().find(|variant| variant.id == *id))
            .or_else(|| variants.iter().find(|variant| !owned.contains(&variant.id)))
            .or_else(|| variants.first())
            .cloned()
            .unwrap_or(StoreVariant {
                id: item.id,
                variant_name: Some(item.name.clone()),
                model_variant: None,
                variant_order: 0,
                asset_id: item.asset_id,
                cover_asset_id: item.cover_asset_id,
                preview_url: item.preview_url.clone(),
            })
    }

    pub fn fully_owned(&self, item: &StoreItem) -> bool {
        let owned = self.owned_ids();
        item.ui_variants()
            .iter()
            .all(|variant| owned.contains(&variant.id))
    }

    pub fn shop(&self, kind: CosmeticType) -> Vec<StoreItem> {
        let items: Vec<StoreItem> = self
            .store
            .as_ref()
            .map(|store| store.of_type(kind).to_vec())
            .unwrap_or_default()
            .into_iter()
            .filter(|item| !self.fully_owned(item) && self.matches(&item.name))
            .collect();
        self.sorted(items)
    }

    fn sorted(&self, mut items: Vec<StoreItem>) -> Vec<StoreItem> {
        match self.sort {
            Sort::Popular => {}
            Sort::Newest => items.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
            Sort::PriceLow => items.sort_by(|a, b| {
                a.final_price()
                    .unwrap_or(0.)
                    .total_cmp(&b.final_price().unwrap_or(0.))
            }),
            Sort::PriceHigh => items.sort_by(|a, b| {
                b.final_price()
                    .unwrap_or(0.)
                    .total_cmp(&a.final_price().unwrap_or(0.))
            }),
            Sort::Name => items.sort_by_key(|item| item.name.to_lowercase()),
        }
        items
    }

    pub fn store_item(&self, id: i32) -> Option<&StoreItem> {
        self.store.as_ref().and_then(|store| store.find(id))
    }

    pub fn bundle(&self, id: i32) -> Option<&BundleInfo> {
        self.store
            .as_ref()
            .and_then(|store| store.bundles.iter().find(|bundle| bundle.id == id))
    }

    pub fn cover_for_variant(&self, variant: i32) -> Option<String> {
        let item = self.store.as_ref()?.containing_variant(variant)?;
        item.variants
            .iter()
            .flatten()
            .find(|entry| entry.id == variant)
            .and_then(|entry| entry.cover_asset_id)
            .or(item.cover_asset_id)
            .map(api::asset_url)
    }

    pub fn preview_for_variant(&self, variant: i32) -> Option<String> {
        let item = self.store.as_ref()?.containing_variant(variant)?;
        item.variants
            .iter()
            .flatten()
            .find(|entry| entry.id == variant)
            .and_then(|entry| entry.preview_url.clone())
            .or_else(|| item.preview_url.clone())
    }

    pub fn variant_cover(&self, variant: &CosmeticVariant) -> Option<String> {
        variant
            .cover_url
            .clone()
            .or_else(|| self.cover_for_variant(variant.id))
    }

    pub fn mojang_cape(&self, id: &str) -> Option<&MojangCape> {
        self.look
            .as_ref()
            .and_then(|look| look.capes.iter().find(|cape| cape.id == id))
    }

    pub fn current_cape(&self) -> Option<CapeSource> {
        let plus_cape = self
            .plus
            .as_ref()
            .and_then(|plus| plus.equipped.get(&BodySlot::Cape))
            .and_then(|id| self.variant(*id))
            .and_then(|(_, variant)| variant.url.clone())
            .map(CapeSource::Plus);
        plus_cape.or_else(|| {
            self.look
                .as_ref()
                .and_then(active_cape)
                .map(|cape| CapeSource::Mojang(cape.url.clone()))
        })
    }

    pub fn current_skin_hash(&self) -> Option<String> {
        self.look
            .as_ref()
            .and_then(active_skin)
            .and_then(|skin| texture_hash(&skin.url))
    }

    pub fn skin(&self, id: i32) -> Option<&SkinInfo> {
        self.skins
            .iter()
            .flat_map(|list| list.skins.iter())
            .find(|skin| skin.id == id)
    }

    pub fn particle_color(&self) -> Option<i32> {
        self.plus.as_ref().and_then(|plus| plus.particle_color)
    }

    pub fn outfit(&self, id: i32) -> Option<&OutfitInfo> {
        self.outfits
            .iter()
            .flat_map(|list| list.outfits.iter())
            .find(|outfit| outfit.id == id)
    }

    pub fn outfit_look(&self, outfit: &OutfitInfo) -> (Option<(String, bool)>, Option<CapeSource>) {
        let skin = outfit
            .skin_id
            .and_then(|id| self.skin(id))
            .map(|skin| (skin.url.clone(), skin.model == SkinModel::Slim));
        let plus_cape = outfit
            .equipped
            .get(&BodySlot::Cape)
            .and_then(|id| self.variant(*id))
            .and_then(|(_, variant)| variant.url.clone())
            .map(CapeSource::Plus);
        let cape = plus_cape.or_else(|| {
            outfit
                .mojang_cape_id
                .as_deref()
                .and_then(|id| self.mojang_cape(id))
                .map(|cape| CapeSource::Mojang(cape.url.clone()))
        });
        (skin, cape)
    }

    pub fn look_cosmetics(
        &self,
        equipped: &HashMap<BodySlot, i32>,
        try_on: Vec<(BodySlot, String)>,
    ) -> Vec<(BodySlot, String)> {
        let tried: Vec<BodySlot> = try_on.iter().map(|(slot, _)| *slot).collect();
        let mut cosmetics: Vec<(BodySlot, String)> = equipped
            .iter()
            .filter(|(slot, _)| !tried.contains(slot))
            .filter_map(|(slot, id)| Some((*slot, self.variant(*id)?.1.url.clone()?)))
            .chain(try_on)
            .filter(|(slot, _)| *slot != BodySlot::Cape)
            .collect();
        cosmetics.sort();
        cosmetics
    }

    pub fn equipped(&self) -> HashMap<BodySlot, i32> {
        self.plus
            .as_ref()
            .map(|plus| plus.equipped.clone())
            .unwrap_or_default()
    }

    pub fn outfits_in_order(&self) -> Vec<OutfitInfo> {
        favorites_first(
            self.outfits
                .as_ref()
                .map_or(&[][..], |list| list.outfits.as_slice()),
            |outfit| outfit.favorite,
        )
    }
}

pub(super) fn type_label(kind: CosmeticType) -> &'static str {
    match kind {
        CosmeticType::Emote => t("oneclient.wardrobe.type.emote"),
        _ => categories()
            .into_iter()
            .find(|(category, _)| *category == kind)
            .map_or(t("oneclient.wardrobe.type.cosmetic"), |(_, label)| label),
    }
}

pub(super) fn collection_label(kind: CosmeticType) -> String {
    tf("oneclient.wardrobe.collection", &[&type_label(kind)])
}

pub(super) fn cosmetic_count_label(count: usize) -> String {
    match count {
        0 => t("oneclient.wardrobe.cosmetic_count.none").into(),
        1 => tf("oneclient.wardrobe.cosmetic_count.one", &[&count]),
        n => tf("oneclient.wardrobe.cosmetic_count.other", &[&n]),
    }
}

pub(super) fn variant_label(variant: &StoreVariant, item: &StoreItem) -> String {
    variant
        .variant_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| item.name.clone())
}

pub(super) fn cart_name(item: &StoreItem, variant: &StoreVariant) -> String {
    let name = variant_label(variant, item);
    if item.ui_variants().len() > 1 && !name.eq_ignore_ascii_case(&item.name) {
        format!("{} - {name}", item.name)
    } else {
        item.name.clone()
    }
}

pub(super) fn is_new(created_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(created_at).is_ok_and(|created| {
        chrono::Utc::now().signed_duration_since(created) < chrono::TimeDelta::days(7)
    })
}

pub(super) fn shop_thumbnail(
    data: &WardrobeData,
    item: &StoreItem,
    variant: &StoreVariant,
    interactive: bool,
) -> Element {
    // Cards show the generated cover; only the big preview renders the cape on the player
    if interactive
        && item.kind == CosmeticType::Cape
        && let Some(asset) = variant.asset_id.or(item.asset_id)
    {
        return LookThumb {
            uuid: data.uuid.clone(),
            skin: None,
            cape: Some(CapeSource::Plus(api::asset_url(asset))),
            cosmetics: Vec::new(),
            emote: None,
            particle_color: None,
            yaw: BACK_YAW,
            zoom: 1.0,
            interactive,
        }
        .into_element();
    }
    if interactive && let Some(asset) = variant.asset_id.or(item.asset_id) {
        let url = api::asset_url(asset);
        let emote = item.kind == CosmeticType::Emote;
        let try_on = item.kind.default_slot().map(|slot| (slot, url.clone()));
        return LookThumb {
            uuid: data.uuid.clone(),
            skin: None,
            cape: data.current_cape(),
            cosmetics: data.look_cosmetics(&data.equipped(), try_on.into_iter().collect()),
            emote: emote.then_some(url),
            particle_color: data.particle_color(),
            yaw: if BACK_VIEW_TYPES.contains(&item.kind) {
                BACK_YAW
            } else {
                FRONT_YAW
            },
            zoom: 1.0,
            interactive,
        }
        .into_element();
    }
    CoverThumb {
        url: variant
            .cover_asset_id
            .or(item.cover_asset_id)
            .map(api::asset_url),
    }
    .into_element()
}

pub(super) fn equip_slot(group: &CosmeticGroup, variant: &CosmeticVariant) -> Option<BodySlot> {
    // A variant's own slots win: a group can list both hands while each variant fits only one
    let slots = if variant.allowed_slots.is_empty() {
        &group.allowed_slots
    } else {
        &variant.allowed_slots
    };
    group
        .kind
        .default_slot()
        .filter(|slot| slots.is_empty() || slots.contains(slot))
        .or_else(|| {
            slots
                .iter()
                .copied()
                .find(|slot| *slot != BodySlot::Unknown)
        })
}

/// Variants by the slot each is worn in; Poly+'s "Both" fills two slots at once
pub(super) type Pick = Vec<(BodySlot, i32)>;

pub(super) fn variant_pick(group: &CosmeticGroup, variant: &CosmeticVariant) -> Pick {
    equip_slot(group, variant)
        .map(|slot| vec![(slot, variant.id)])
        .unwrap_or_default()
}

/// The hand a variant is limited to, if only one
fn hand_side(group: &CosmeticGroup, variant: &CosmeticVariant) -> Option<BodySlot> {
    let allows = |slot| {
        if variant.allowed_slots.is_empty() {
            group.kind == CosmeticType::Glove
        } else {
            variant.allowed_slots.contains(&slot)
        }
    };
    match (allows(BodySlot::LeftHand), allows(BodySlot::RightHand)) {
        (true, false) => Some(BodySlot::LeftHand),
        (false, true) => Some(BodySlot::RightHand),
        _ => None,
    }
}

/// The variant chips as Poly+ builds them: one per name and hand, "Left"/"Right" added when both
/// hands share a name, and a "Both" chip after each left and right pair
pub(super) fn variant_options(group: &CosmeticGroup) -> Vec<(String, Pick)> {
    let mut chips: Vec<(&str, Option<BodySlot>, &CosmeticVariant)> = Vec::new();
    for variant in &group.variants {
        let side = hand_side(group, variant);
        if !chips
            .iter()
            .any(|(name, other, _)| *name == variant.name && *other == side)
        {
            chips.push((&variant.name, side, variant));
        }
    }
    let labelled: Vec<(String, Option<BodySlot>, &CosmeticVariant)> = chips
        .iter()
        .map(|&(name, side, variant)| {
            let shared = |other| {
                chips
                    .iter()
                    .any(|(n, s, _)| *n == name && *s == Some(other))
            };
            let label = match side {
                Some(BodySlot::LeftHand) if shared(BodySlot::RightHand) => format!("Left {name}"),
                Some(BodySlot::RightHand) if shared(BodySlot::LeftHand) => format!("Right {name}"),
                _ => name.to_string(),
            };
            (label, side, variant)
        })
        .collect();

    let finish = |name: &str| {
        name.split(' ')
            .filter(|word| {
                !word.eq_ignore_ascii_case("left") && !word.eq_ignore_ascii_case("right")
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut done: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for (label, side, variant) in &labelled {
        if side.is_none() {
            out.push((label.clone(), variant_pick(group, variant)));
            continue;
        }
        let rest = finish(label);
        if done.contains(&rest) {
            continue;
        }
        let find = |want| {
            labelled
                .iter()
                .find(|(l, s, _)| *s == Some(want) && finish(l) == rest)
        };
        let left = find(BodySlot::LeftHand);
        let right = find(BodySlot::RightHand);
        for (label, side, variant) in left.iter().chain(right.iter()) {
            out.push((
                label.clone(),
                vec![(side.unwrap_or(BodySlot::RightHand), variant.id)],
            ));
        }
        if let (Some((_, _, left)), Some((_, _, right))) = (left, right) {
            let label = if rest.is_empty() {
                t("oneclient.wardrobe.preview.both").to_string()
            } else {
                tf("oneclient.wardrobe.preview.both_named", &[&rest])
            };
            out.push((
                label,
                vec![
                    (BodySlot::LeftHand, left.id),
                    (BodySlot::RightHand, right.id),
                ],
            ));
        }
        done.push(rest);
    }
    out
}

/// Slot and variant name per piece, so slim/wide copies of a variant count as the same pick
pub(super) fn pick_key(group: &CosmeticGroup, pick: &Pick) -> Vec<(BodySlot, String)> {
    let mut key: Vec<_> = pick
        .iter()
        .filter_map(|(slot, id)| {
            let variant = group.variants.iter().find(|variant| variant.id == *id)?;
            Some((*slot, variant.name.clone()))
        })
        .collect();
    key.sort();
    key
}

/// Equips the picked variants, clearing any other slot the group is in, or unequips when the pick is already worn
pub(super) fn toggle_owned(
    data: &WardrobeData,
    group: &CosmeticGroup,
) -> Result<WardrobeAction, String> {
    let worn = data.worn(group);
    let pick = data.resolved_pick(group);
    if pick.is_empty() {
        return Err(tf("oneclient.wardrobe.error.cannot_equip", &[&group.name]));
    }
    let changes = if worn == pick {
        worn.iter().map(|(slot, _)| (*slot, None)).collect()
    } else {
        worn.iter()
            .filter(|(slot, _)| !pick.iter().any(|(picked, _)| picked == slot))
            .map(|(slot, _)| (*slot, None))
            .chain(pick.iter().map(|(slot, id)| (*slot, Some(*id))))
            .collect()
    };
    Ok(WardrobeAction::Equip {
        account: data.account,
        changes,
        name: group.name.clone(),
    })
}

pub(super) fn toggle_mojang_action(data: &WardrobeData, cape: &MojangCape) -> WardrobeAction {
    let active = cape.state.eq_ignore_ascii_case("ACTIVE");
    WardrobeAction::MojangCape {
        account: data.account,
        cape: (!active).then(|| cape.id.clone()),
        name: cape.alias.clone(),
    }
}

fn favorites_first<T: Clone>(items: &[T], favorite: impl Fn(&T) -> bool) -> Vec<T> {
    let mut sorted = items.to_vec();
    sorted.sort_by_key(|item| !favorite(item));
    sorted
}

fn report(mut status: State<Option<String>>, message: String) {
    if let Some(mut status) = status.try_write() {
        *status = Some(message);
    }
}

pub(super) fn perform(
    action: UseWardrobeAction,
    status: State<Option<String>>,
    keys: WardrobeAction,
) {
    perform_then(action, status, keys, || {});
}

pub(super) fn perform_then(
    action: UseWardrobeAction,
    mut status: State<Option<String>>,
    keys: WardrobeAction,
    on_ok: impl FnOnce() + 'static,
) {
    if mutation_is_running(&action) {
        status.set(Some(t("oneclient.wardrobe.status.busy").into()));
        return;
    }
    if let Some(pending) = keys.pending() {
        status.set(Some(pending));
    }
    spawn_in_window(async move {
        let reader = action.mutate_async(keys).await;
        let outcome = match &*reader.state() {
            MutationStateData::Settled { res, .. } => Some(res.clone()),
            _ => None,
        };
        match outcome {
            Some(Ok(message)) => {
                report(status, message);
                on_ok();
            }
            Some(Err(message)) => report(status, message),
            None => {}
        }
    });
}

pub(super) fn add_to_cart(
    already_owned: bool,
    mut cart: State<Vec<CartEntry>>,
    mut status: State<Option<String>>,
    item: &StoreItem,
    variant: &StoreVariant,
) {
    let key = cart_key(variant.id);
    let name = cart_name(item, variant);
    if already_owned {
        status.set(Some(tf("oneclient.wardrobe.cart.already_owned", &[&name])));
        return;
    }
    if cart.read().iter().any(|entry| entry.key == key) {
        status.set(Some(tf(
            "oneclient.wardrobe.cart.already_in_cart",
            &[&name],
        )));
        return;
    }
    status.set(Some(tf("oneclient.wardrobe.cart.adding", &[&name])));
    let description = item.description.clone();
    let base_price = item.base_price;
    let final_price = item.final_price();
    let discount_percent = item.discount_percent();
    let cover_asset_id = variant.cover_asset_id.or(item.cover_asset_id);
    let variant_id = variant.id;
    spawn_in_window(async move {
        let message = match api::store_product_id(variant_id).await {
            Ok(Some(product_id)) => {
                if cart
                    .try_peek()
                    .is_some_and(|entries| entries.iter().all(|entry| entry.key != key))
                    && let Some(mut entries) = cart.try_write()
                {
                    entries.push(CartEntry {
                        key,
                        name: name.clone(),
                        description,
                        base_price,
                        final_price,
                        discount_percent,
                        cosmetic: variant_id,
                        product_id,
                        cover_asset_id,
                    });
                }
                tf("oneclient.wardrobe.cart.added", &[&name])
            }
            Ok(None) => tf("oneclient.wardrobe.not_purchasable", &[&name]),
            Err(err) => plus_message(err),
        };
        report(status, message);
    });
}

#[derive(PartialEq)]
pub struct Wardrobe;

impl Component for Wardrobe {
    fn render(&self) -> impl IntoElement {
        let current = use_current_account();
        let account = try_default_account(&current);

        let body = match account {
            Some(account) if account.is_microsoft() => WardrobeBody {
                account: account.id,
            }
            .into_element(),
            Some(_) => centered_note(t("oneclient.wardrobe.account.not_microsoft")),
            None if query_is_loading(&current) => {
                centered_spinner(t("oneclient.wardrobe.account.loading"))
            }
            None => centered_note(t("oneclient.wardrobe.account.signed_out")),
        };

        rect().width(Size::fill()).height(Size::fill()).child(body)
    }
}

#[derive(Clone, PartialEq, Default)]
struct Anchors {
    scroll: f32,
    pinned: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) struct Shared {
    pub selection: State<Option<Selection>>,
    pub trying_on: State<bool>,
    pub adding_skin: State<bool>,
    pub show_cart: State<bool>,
    pub cart: State<Vec<CartEntry>>,
    pub variant_picks: State<HashMap<i32, Pick>>,
    pub store_picks: State<HashMap<i32, i32>>,
    pub status: State<Option<String>>,
    pub deleting: State<Option<(Selection, String, WardrobeAction)>>,
    pub action: UseWardrobeAction,
}

impl PartialEq for Shared {
    fn eq(&self, other: &Self) -> bool {
        self.selection == other.selection
            && self.trying_on == other.trying_on
            && self.adding_skin == other.adding_skin
            && self.show_cart == other.show_cart
            && self.cart == other.cart
            && self.variant_picks == other.variant_picks
            && self.store_picks == other.store_picks
            && self.status == other.status
            && self.deleting == other.deleting
    }
}

impl Shared {
    pub fn run(&self, keys: WardrobeAction) -> EventHandler<()> {
        let action = self.action;
        let status = self.status;
        EventHandler::new(move |()| perform(action, status, keys.clone()))
    }
}

#[derive(Clone, Copy)]
struct WardrobeQueries {
    look: UseQuery<MojangLookQuery>,
    plus: UseQuery<PlusCosmeticsQuery>,
    outfits: UseQuery<OutfitsQuery>,
    skins: UseQuery<SkinsQuery>,
    store: UseQuery<StoreQuery>,
}

fn settled_error<Q: QueryCapability<Err = String>>(query: &UseQuery<Q>) -> Option<String> {
    let reader = query.peek();
    let state = reader.state();
    state.err().cloned()
}

impl WardrobeQueries {
    fn error(&self) -> Option<String> {
        settled_error(&self.look)
            .or_else(|| settled_error(&self.plus))
            .or_else(|| settled_error(&self.outfits))
            .or_else(|| settled_error(&self.skins))
            .or_else(|| settled_error(&self.store))
    }
}

#[derive(PartialEq)]
struct WardrobeBody {
    account: Uuid,
}

impl WardrobeBody {
    fn key_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.account.hash(&mut hasher);
        hasher.finish()
    }
}

impl Component for WardrobeBody {
    fn render(&self) -> impl IntoElement {
        let account = self.account;
        let look_query = use_mojang_look(account);
        let plus_query = use_plus_cosmetics(account);
        let outfits_query = use_outfits(account);
        let skins_query = use_skin_library(account);
        let store_query = use_store();
        let action = use_wardrobe_action();

        let section = use_state(|| Section::Cosmetics);
        let show_history = use_state(|| false);
        let sort = use_state(|| Sort::Popular);
        let query = use_state(String::new);
        let mut grid_width = use_state(|| 0f32);
        let anchors = use_state(Anchors::default);
        let refreshing = use_state(|| false);
        let mut last_plus = use_state(|| None::<PlayerCosmetics>);
        let scroll = use_scroll_controller(ScrollConfig::default);
        let shared = Shared {
            selection: use_state(|| None::<Selection>),
            trying_on: use_state(|| false),
            adding_skin: use_state(|| false),
            show_cart: use_state(|| false),
            cart: use_state(Vec::<CartEntry>::new),
            variant_picks: use_state(HashMap::<i32, Pick>::new),
            store_picks: use_state(HashMap::<i32, i32>::new),
            status: use_state(|| None::<String>),
            deleting: use_state(|| None::<(Selection, String, WardrobeAction)>),
            action,
        };

        let look = settled_or_loading(&look_query);
        let plus = settled_or_loading(&plus_query);
        if let Some(current) = &plus
            && last_plus.peek().as_ref() != Some(current)
        {
            last_plus.set(Some(current.clone()));
        }
        let plus = plus.or_else(|| last_plus.peek().clone());
        let data = WardrobeData {
            account,
            uuid: account.to_string(),
            slim: look
                .as_ref()
                .and_then(active_skin)
                .is_some_and(|skin| skin.variant == oneclient_mc::SkinVariant::Slim),
            look,
            look_error: query_error(&look_query),
            owned: owned_set(plus.as_ref()),
            plus,
            plus_error: query_error(&plus_query),
            outfits: settled_or_loading(&outfits_query),
            outfits_error: query_error(&outfits_query),
            skins: settled_or_loading(&skins_query),
            skins_error: query_error(&skins_query),
            store: settled_or_loading(&store_query),
            store_error: query_error(&store_query),
            busy: mutation_is_running(&action),
            grid_width: *grid_width.read(),
            sort: *sort.read(),
            query: query.read().clone(),
            variant_picks: shared.variant_picks.read().clone(),
            store_picks: shared.store_picks.read().clone(),
            cart_keys: shared
                .cart
                .read()
                .iter()
                .map(|entry| entry.key.clone())
                .collect(),
        };

        let mut owned_ids: Vec<i32> = data.owned.iter().copied().collect();
        owned_ids.sort_unstable();
        let mut pruned_cart = shared.cart;
        use_side_effect_with_deps(&owned_ids, move |owned: &Vec<i32>| {
            let stale = pruned_cart
                .peek()
                .iter()
                .any(|entry| owned.binary_search(&entry.cosmetic).is_ok());
            if stale {
                pruned_cart
                    .write()
                    .retain(|entry| owned.binary_search(&entry.cosmetic).is_err());
            }
        });

        let loading = query_is_loading(&look_query)
            || query_is_loading(&plus_query)
            || query_is_loading(&store_query);
        let reveal = use_animation_with_dependencies(&loading, |conf, _| {
            conf.on_creation(OnCreation::Run);
            conf.on_change(OnChange::Rerun);
            AnimNum::new(0., 1.)
                .time(260)
                .ease(Ease::Out)
                .function(Function::Cubic)
        });
        if loading {
            return centered_spinner(t("oneclient.wardrobe.page.loading"));
        }

        let current_section = if data.plus_available() {
            *section.read()
        } else {
            Section::Cosmetics
        };
        let history = *show_history.read() && data.plus_available();
        let ctx = GridCtx {
            data: &data,
            shared,
        };
        let mut blocks = if history {
            Vec::new()
        } else {
            match current_section {
                Section::Cosmetics => cosmetics_content(&ctx),
                Section::Outfits => outfits_content(&ctx),
                Section::Skins => skins_content(&ctx),
            }
        };
        if !history
            && data.plus.is_some()
            && let Some(error) = &data.plus_error
        {
            blocks.insert(
                0,
                Block::notice(tf("oneclient.wardrobe.plus_refresh_failed", &[error])),
            );
        }
        let layout = Rc::new(layout(blocks, data.grid_width));
        let labels = layout.anchors.clone();

        let mut anchors_state = anchors;
        let grid = ScrollArea::new()
            .width(Size::fill())
            .height(Size::flex(1.0))
            .scroll_controller(scroll)
            .reset_key(section_key(current_section, history))
            .on_user_scroll(move |()| {
                if anchors_state.peek().pinned.is_some() {
                    anchors_state.write().pinned = None;
                }
            })
            .on_ctx(move |view: ScrollAreaCtx| {
                if (anchors_state.peek().scroll - view.corrected_y).abs() > 0.5 {
                    anchors_state.write().scroll = view.corrected_y;
                }
            });
        let measured = move |event: Event<SizedEventData>| {
            let width = event.data().area.width() - 8.;
            if (width - *grid_width.peek()).abs() > 0.5 {
                grid_width.set(width);
            }
        };
        let grid = if history {
            grid.child(
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(0., 8., 12., 0.))
                    .on_sized(measured)
                    .child(HistoryList { account }),
            )
        } else {
            let grid_data = data.clone();
            grid.content(move |view: ScrollAreaCtx| {
                let ctx = GridCtx {
                    data: &grid_data,
                    shared,
                };
                rect()
                    .width(Size::fill())
                    .padding(Gaps::new(0., 8., 12., 0.))
                    .on_sized(measured)
                    .child(visible_slots(&ctx, &layout, &view))
                    .into_element()
            })
        };

        let header = Header {
            data: data.clone(),
            section,
            current: current_section,
            shared,
            show_history,
            sort,
            labels,
            anchors,
            scroll,
            refreshing,
            queries: WardrobeQueries {
                look: look_query,
                plus: plus_query,
                outfits: outfits_query,
                skins: skins_query,
                store: store_query,
            },
        };

        let mut deleting = shared.deleting;
        let mut selection = shared.selection;
        let confirm_overlay = deleting.read().clone().map(|(picked, name, keys)| {
            confirm_dialog(
                tf("oneclient.wardrobe.delete.title", &[&name]),
                t("oneclient.wardrobe.delete.body").to_string(),
                move || deleting.set(None),
                move || {
                    deleting.set(None);
                    if *selection.peek() == Some(picked.clone()) {
                        selection.set(None);
                    }
                    perform(shared.action, shared.status, keys.clone());
                },
            )
        });

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .horizontal()
            .main_align(Alignment::Center)
            .padding(Gaps::new(8., 40., 32., 40.))
            .opacity(reveal.get().value())
            .child(
                rect()
                    .vertical()
                    .width(Size::fill())
                    .height(Size::fill())
                    .content(Content::Flex)
                    .spacing(16.)
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .content(Content::Flex)
                            .cross_align(Alignment::Center)
                            .child(
                                label()
                                    .text(t("oneclient.wardrobe.title"))
                                    .width(Size::flex(1.0))
                                    .font_size(text_size(24.))
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(StoreSearch {
                                data: data.clone(),
                                query,
                                shared,
                                suggest: data.plus_available()
                                    && current_section == Section::Cosmetics,
                            }),
                    )
                    .child(
                        rect()
                            .horizontal()
                            .width(Size::fill())
                            .height(Size::flex(1.0))
                            .content(Content::Flex)
                            .spacing(16.)
                            .child(
                                rect()
                                    .vertical()
                                    .width(Size::flex(1.0))
                                    .height(Size::fill())
                                    .content(Content::Flex)
                                    .spacing(12.)
                                    .child(header.render())
                                    .child(grid),
                            )
                            .child(
                                rect()
                                    .width(Size::percent(22.))
                                    .min_width(Size::px(RIGHT_COLUMN_MIN_WIDTH))
                                    .height(Size::fill())
                                    .child(RightColumn {
                                        data: data.clone(),
                                        section: current_section,
                                        shared,
                                    }),
                            ),
                    ),
            )
            .maybe_child(confirm_overlay)
            .into_element()
    }

    fn render_key(&self) -> DiffKey {
        DiffKey::U64(self.key_hash())
    }
}

const SEARCH_WIDTH: f32 = 260.;
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);

/// The search box, plus a live dropdown of store matches while typing on the
/// cosmetics tab. The grid below still filters by the same text.
#[derive(PartialEq)]
struct StoreSearch {
    data: WardrobeData,
    query: State<String>,
    shared: Shared,
    suggest: bool,
}

impl Component for StoreSearch {
    fn render(&self) -> impl IntoElement {
        let query = self.query;
        let text = query.read().trim().to_string();
        let settled = use_debounced(text.clone(), SEARCH_DEBOUNCE);
        let settled = settled.read().clone();
        let suggestions = use_store_suggestions(if self.suggest { &settled } else { "" });
        let mut open = use_state(|| false);
        let mut input_area = use_state(|| None::<Area>);
        let mut panel_area = use_state(|| None::<Area>);

        use_side_effect_with_deps(&text, move |text: &String| {
            open.set_if_modified(!text.is_empty());
        });
        let showing = self.suggest && *open.read() && !text.is_empty();
        use_overlay_claim_when(showing);

        let on_global_pointer_up = move |e: Event<PointerEventData>| {
            let point = e.global_location();
            let point = (point.x as f32, point.y as f32).into();
            let inside = |area: Option<Area>| area.is_some_and(|area| area.contains(point));
            if !inside(*input_area.peek()) && !inside(*panel_area.peek()) {
                open.set_if_modified(false);
            }
        };

        let panel = showing.then(|| {
            let mut rows = rect().vertical().width(Size::fill()).spacing(2.);
            let searching = text != settled || query_is_loading(&suggestions);
            let results = settled_or_loading(&suggestions).unwrap_or_default();
            if searching && results.is_empty() {
                rows = rows.child(search_note(
                    Some(Spinner::new(14.).into_element()),
                    t("oneclient.wardrobe.search.searching").to_string(),
                ));
            } else if let Some(error) = query_error(&suggestions).filter(|_| results.is_empty()) {
                rows = rows.child(search_note(None, error));
            } else if results.is_empty() {
                rows = rows.child(search_note(
                    None,
                    tf("oneclient.wardrobe.search.no_results", &[&text]),
                ));
            } else {
                let ctx = GridCtx {
                    data: &self.data,
                    shared: self.shared,
                };
                for item in results {
                    let select = ctx.select(suggestion_selection(&self.data, item.id));
                    rows = rows.child(SuggestionRow {
                        item,
                        on_press: EventHandler::new(move |()| {
                            open.set(false);
                            select.call(());
                        }),
                    });
                }
                rows = rows.child(
                    rect()
                        .width(Size::fill())
                        .padding(Gaps::new(8., 10., 6., 10.))
                        .cursor(CursorIcon::Pointer)
                        .a11y_role(AccessibilityRole::Button)
                        .on_press(move |_| open.set(false))
                        .child(
                            label()
                                .text(tf("oneclient.wardrobe.search.see_all", &[&text]))
                                .max_lines(1)
                                .text_overflow(TextOverflow::Ellipsis)
                                .font_size(text_size(12.))
                                .color(colors::fg_secondary()),
                        ),
                );
            }
            rect().width(Size::px(0.)).height(Size::px(0.)).child(
                rect()
                    .layer(Layer::Overlay)
                    .width(Size::px(SEARCH_WIDTH))
                    .margin(Gaps::new(4., 0., 0., 0.))
                    .padding(4.)
                    .corner_radius(rounded(8.))
                    .border(border_all_color(1., colors::component_border()))
                    .background(colors::page_elevated())
                    .on_sized(move |e: Event<SizedEventData>| {
                        panel_area.set_if_modified(Some(e.area));
                    })
                    .child(rows),
            )
        });

        rect()
            .width(Size::px(SEARCH_WIDTH))
            .on_global_pointer_up(on_global_pointer_up)
            .on_global_key_down(move |e: Event<KeyboardEventData>| {
                if e.key == Key::Named(NamedKey::Escape) {
                    open.set_if_modified(false);
                }
            })
            .child(
                rect()
                    .width(Size::fill())
                    .on_sized(move |e: Event<SizedEventData>| {
                        input_area.set_if_modified(Some(e.area));
                    })
                    .on_press(move |_| {
                        if !query.peek().trim().is_empty() {
                            open.set_if_modified(true);
                        }
                    })
                    .child(
                        TextInput::new(query)
                            .placeholder(t("oneclient.wardrobe.search"))
                            .compact()
                            .width(Size::fill())
                            .on_submit(move |_| open.set(false))
                            .leading(
                                Icon::new(IconType::SearchMd)
                                    .size(14.)
                                    .color(colors::fg_secondary())
                                    .into_element(),
                            ),
                    ),
            )
            .maybe_child(panel)
    }
}

/// Owned items open as owned so the preview offers equip instead of buy
fn suggestion_selection(data: &WardrobeData, id: i32) -> Selection {
    if data.emotes().iter().any(|emote| emote.id == id) {
        Selection::Emote(id)
    } else if let Some((group, _)) = data.variant(id) {
        Selection::Owned(group.id)
    } else {
        Selection::Shop(id)
    }
}

fn search_note(leading: Option<Element>, text: String) -> Element {
    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new_symmetric(10., 10.))
        .cross_align(Alignment::Center)
        .spacing(8.)
        .maybe_child(leading)
        .child(
            label()
                .text(text)
                .max_lines(1)
                .text_overflow(TextOverflow::Ellipsis)
                .font_size(text_size(13.))
                .color(colors::fg_secondary()),
        )
        .into_element()
}

#[derive(PartialEq)]
struct SuggestionRow {
    item: api::StoreSuggestion,
    on_press: EventHandler<()>,
}

impl Component for SuggestionRow {
    fn render(&self) -> impl IntoElement {
        let mut hovered = use_state(|| false);
        let on_press = self.on_press.clone();
        rect()
            .horizontal()
            .width(Size::fill())
            .padding(6.)
            .spacing(10.)
            .cross_align(Alignment::Center)
            .corner_radius(rounded(6.))
            .background(if *hovered.read() {
                colors::ghost_overlay_hover()
            } else {
                Color::TRANSPARENT
            })
            .cursor(CursorIcon::Pointer)
            .a11y_role(AccessibilityRole::Button)
            .a11y_alt(self.item.name.clone())
            .on_pointer_enter(move |_| hovered.set(true))
            .on_pointer_leave(move |_| hovered.set(false))
            .on_press(move |_| on_press.call(()))
            .child(
                rect()
                    .width(Size::px(36.))
                    .height(Size::px(36.))
                    .corner_radius(rounded(5.))
                    .background(colors::page())
                    .overflow(Overflow::Clip)
                    .child(CoverThumb {
                        url: self.item.cover_asset_id.map(api::asset_url),
                    }),
            )
            .child(
                rect()
                    .vertical()
                    .width(Size::flex(1.0))
                    .spacing(2.)
                    .child(
                        label()
                            .text(self.item.name.clone())
                            .max_lines(1)
                            .text_overflow(TextOverflow::Ellipsis)
                            .font_size(text_size(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .color(colors::fg_primary()),
                    )
                    .child(
                        label()
                            .text(type_label(self.item.kind))
                            .max_lines(1)
                            .font_size(text_size(11.))
                            .color(colors::fg_secondary()),
                    ),
            )
    }
}

fn section_key(section: Section, history: bool) -> u64 {
    let base = match section {
        Section::Cosmetics => 1,
        Section::Outfits => 2,
        Section::Skins => 3,
    };
    if history { base + 10 } else { base }
}

struct AnchorRow {
    labels: Vec<(String, f32)>,
    anchors: State<Anchors>,
    scroll: ScrollController,
}

impl PartialEq for AnchorRow {
    fn eq(&self, other: &Self) -> bool {
        self.labels == other.labels && self.anchors == other.anchors
    }
}

impl Component for AnchorRow {
    fn render(&self) -> impl IntoElement {
        let labels = &self.labels;
        let anchors = self.anchors;
        let mut scroll = self.scroll;
        let state = anchors.read();
        let position = -state.scroll + 8.;
        let pinned = state
            .pinned
            .clone()
            .filter(|text| labels.iter().any(|(label, _)| label == text));
        let active = pinned.or_else(|| {
            labels
                .iter()
                .rev()
                .find(|(_, offset)| *offset <= position)
                .or_else(|| labels.first())
                .map(|(text, _)| text.clone())
        });
        let mut row = rect()
            .horizontal()
            .cross_align(Alignment::Center)
            .spacing(4.);
        if labels.len() > 1 {
            for (text, offset) in labels {
                let offset = *offset;
                let mut anchors = anchors;
                let name = text.clone();
                row = row.child(AnchorChip {
                    text: text.clone(),
                    active: active.as_deref() == Some(text.as_str()),
                    on_press: EventHandler::new(move |()| {
                        anchors.write().pinned = Some(name.clone());
                        scroll.scroll_to_y(-(offset.round() as i32));
                    }),
                });
            }
        }
        row
    }
}

struct Header {
    data: WardrobeData,
    section: State<Section>,
    current: Section,
    shared: Shared,
    show_history: State<bool>,
    sort: State<Sort>,
    labels: Vec<(String, f32)>,
    anchors: State<Anchors>,
    scroll: ScrollController,
    refreshing: State<bool>,
    queries: WardrobeQueries,
}

impl Header {
    fn render(self) -> Element {
        let Header {
            data,
            mut section,
            current,
            shared,
            mut show_history,
            mut sort,
            labels,
            mut anchors,
            scroll,
            mut refreshing,
            queries,
        } = self;
        let mut adding_skin = shared.adding_skin;
        let mut show_cart = shared.show_cart;
        let store_enabled = data.plus_available();
        let history = *show_history.read() && store_enabled;
        let sections: Vec<(Section, &str, IconType)> = if store_enabled {
            section_tabs().to_vec()
        } else {
            vec![section_tabs()[0]]
        };

        // Scrolls instead of squashing when the sort and cart controls crowd narrow windows
        let mut chips = ScrollView::new()
            .direction(Direction::Horizontal)
            .show_scrollbar(false)
            .invert_scroll_wheel(true)
            .width(Size::flex(1.0))
            .height(Size::px(33.))
            .spacing(8.);
        for (target, text, icon) in sections {
            chips = chips.child(SectionChip {
                text,
                icon,
                selected: current == target && !history,
                on_press: EventHandler::new(move |()| {
                    section.set(target);
                    show_history.set(false);
                    adding_skin.set(false);
                    if anchors.peek().pinned.is_some() {
                        anchors.write().pinned = None;
                    }
                }),
            });
        }

        let cart_size = shared.cart.read().len();
        let sort_labels: Vec<String> = Sort::ALL
            .iter()
            .map(|sort| sort.label().to_string())
            .collect();
        let sort_selected = sort.read().label();
        let mut status = shared.status;
        let top = rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(33.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(chips)
            .maybe_child(
                (store_enabled && !history && current == Section::Cosmetics).then(|| {
                    Dropdown::new(sort_selected, sort_labels)
                        .width(Size::px(132.))
                        .height(Size::px(30.))
                        .leading(
                            Icon::new(IconType::Sliders04)
                                .size(13.)
                                .color(colors::fg_secondary()),
                        )
                        .on_select(move |idx: usize| {
                            if let Some(picked) = Sort::ALL.get(idx) {
                                sort.set(*picked);
                            }
                        })
                }),
            )
            .maybe_child(store_enabled.then(|| {
                Button::new()
                    .secondary()
                    .icon()
                    .tooltip(if history {
                        t("oneclient.wardrobe.history.back")
                    } else {
                        t("oneclient.wardrobe.history.title")
                    })
                    .on_press(move |_| {
                        let open = !*show_history.read();
                        show_history.set(open);
                    })
                    .child(
                        Icon::new(if history {
                            IconType::ArrowLeft
                        } else {
                            IconType::ClockRewind
                        })
                        .size(14.),
                    )
            }))
            .child(
                Button::new()
                    .secondary()
                    .icon()
                    .tooltip(t("oneclient.wardrobe.refresh.tooltip"))
                    .enabled(!*refreshing.read())
                    .on_press(move |_| {
                        if *refreshing.peek() {
                            return;
                        }
                        refreshing.set(true);
                        status.set(Some(t("oneclient.wardrobe.refresh.running").into()));
                        spawn(async move {
                            invalidate_wardrobe_queries().await;
                            let Some(mut refreshing) = refreshing.try_write() else {
                                return;
                            };
                            *refreshing = false;
                            drop(refreshing);
                            report(
                                status,
                                match queries.error() {
                                    Some(error) => {
                                        tf("oneclient.wardrobe.refresh.failed", &[&error])
                                    }
                                    None => t("oneclient.wardrobe.refresh.done").into(),
                                },
                            );
                        });
                    })
                    .child(Icon::new(IconType::RefreshCw01).size(14.)),
            )
            .maybe_child(store_enabled.then(|| {
                Button::new()
                    .primary()
                    .on_press(move |_| {
                        let open = !*show_cart.read();
                        show_cart.set(open);
                    })
                    .text(tf("oneclient.wardrobe.cart.button", &[&cart_size]))
            }));

        let left = if history {
            rect()
                .width(Size::flex(1.0))
                .child(
                    label()
                        .text(t("oneclient.wardrobe.history.title"))
                        .font_size(text_size(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .into_element()
        } else {
            rect()
                .width(Size::flex(1.0))
                .child(
                    // Popular, New, Bundles and Emotes push the chips past narrow windows
                    ScrollView::new()
                        .direction(Direction::Horizontal)
                        .show_scrollbar(false)
                        // so a plain mouse wheel moves the chips sideways
                        .invert_scroll_wheel(true)
                        .width(Size::fill())
                        .height(Size::px(33.))
                        .child(
                            rect()
                                .height(Size::fill())
                                .main_align(Alignment::Center)
                                .child(AnchorRow {
                                    labels,
                                    anchors,
                                    scroll,
                                }),
                        ),
                )
                .into_element()
        };

        let bottom = rect().width(Size::fill()).height(Size::px(33.)).child(left);

        rect()
            .vertical()
            .width(Size::fill())
            .spacing(10.)
            .child(top)
            .child(bottom)
            .into_element()
    }
}

const ANCHOR_HEIGHT: f32 = 24.;
const STORE_LABEL_HEIGHT: f32 = 18.;
const HEADING_HEIGHT: f32 = 28.;
const NOTICE_HEIGHT: f32 = 57.;
const INTRO_HEIGHT: f32 = 33.;
const OVERSCAN: f32 = 480.;
const MIN_COLUMNS: usize = 4;
const MAX_COLUMNS: usize = 6;

#[derive(Clone, Hash)]
pub(super) enum Card {
    Mojang(String),
    Owned(i32),
    Shop(i32),
    Bundle(i32),
    Emote(i32),
    Outfit(i32),
    Skin(i32),
}

pub(super) enum Block {
    Fixed {
        element: Element,
        height: f32,
        anchor: Option<String>,
    },
    Cards(Vec<Card>),
}

impl Block {
    fn fixed(element: Element, height: f32) -> Self {
        Self::Fixed {
            element,
            height,
            anchor: None,
        }
    }

    fn notice(text: impl Into<String>) -> Self {
        Self::fixed(notice(text), NOTICE_HEIGHT)
    }

    fn intro(text: String, button: Option<Element>) -> Self {
        Self::fixed(library_intro(text, button), INTRO_HEIGHT)
    }

    fn heading(title: &str, note: &str) -> Self {
        Self::fixed(
            InfoHeading {
                title: title.into(),
                note: note.into(),
            }
            .into_element(),
            HEADING_HEIGHT,
        )
    }
}

enum SlotContent {
    Fixed(Element),
    Row(Vec<Card>),
}

struct Slot {
    y: f32,
    height: f32,
    content: SlotContent,
}

struct Layout {
    slots: Vec<Slot>,
    anchors: Vec<(String, f32)>,
    total: f32,
    card_width: f32,
}

fn grid_columns(grid_width: f32) -> (usize, f32) {
    if grid_width <= 0. {
        return (MIN_COLUMNS, CARD_WIDTH);
    }
    let columns = (((grid_width + GRID_GAP) / (CARD_WIDTH + GRID_GAP)) as usize)
        .clamp(MIN_COLUMNS, MAX_COLUMNS);
    let width = (grid_width - GRID_GAP * (columns as f32 - 1.)) / columns as f32;
    (columns, width)
}

fn card_element(ctx: &GridCtx, card: &Card) -> Option<Element> {
    let data = ctx.data;
    match card {
        Card::Mojang(id) => data.mojang_cape(id).map(|cape| mojang_card(ctx, cape)),
        Card::Owned(id) => data.group(*id).map(|group| owned_card(ctx, group)),
        Card::Shop(id) => data.store_item(*id).map(|item| shop_card(ctx, item)),
        Card::Bundle(id) => data.bundle(*id).map(|bundle| bundle_card(ctx, bundle)),
        Card::Emote(id) => data
            .emotes()
            .iter()
            .find(|emote| emote.id == *id)
            .map(|emote| emote_card(ctx, emote)),
        Card::Outfit(id) => data.outfit(*id).map(|outfit| outfit_card(ctx, outfit)),
        Card::Skin(id) => data.skin(*id).map(|skin| skin_card(ctx, skin)),
    }
}

fn card_row(ctx: &GridCtx, cards: &[Card], card_width: f32) -> Element {
    let mut row = rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::fill())
        .spacing(GRID_GAP);
    for card in cards {
        let mut hasher = DefaultHasher::new();
        card.hash(&mut hasher);
        row = row.child(
            rect()
                .key(hasher.finish())
                .width(Size::px(card_width))
                .height(Size::fill())
                .maybe_child(card_element(ctx, card)),
        );
    }
    row.into_element()
}

fn layout(blocks: Vec<Block>, grid_width: f32) -> Layout {
    let (columns, card_width) = grid_columns(grid_width);
    let mut slots = Vec::new();
    let mut anchors = Vec::new();
    let mut y = 0.;
    for block in blocks {
        match block {
            Block::Fixed {
                element,
                height,
                anchor,
            } => {
                if let Some(text) = anchor {
                    anchors.push((text, y));
                }
                slots.push(Slot {
                    y,
                    height,
                    content: SlotContent::Fixed(element),
                });
                y += height + GRID_GAP;
            }
            Block::Cards(cards) => {
                for chunk in cards.chunks(columns) {
                    slots.push(Slot {
                        y,
                        height: card_width + CARD_CHROME,
                        content: SlotContent::Row(chunk.to_vec()),
                    });
                    y += card_width + CARD_CHROME + GRID_GAP;
                }
            }
        }
    }
    Layout {
        slots,
        anchors,
        total: y,
        card_width,
    }
}

fn visible_slots(ctx: &GridCtx, layout: &Layout, view: &ScrollAreaCtx) -> Element {
    let top = -view.corrected_y - OVERSCAN;
    let bottom = -view.corrected_y + view.viewport_h + OVERSCAN;
    let slots = &layout.slots;
    let first = slots.partition_point(|slot| slot.y + slot.height < top);
    let last = slots.partition_point(|slot| slot.y <= bottom).max(first);

    let mut column = rect().vertical().width(Size::fill());
    let lead = slots.get(first).map_or(layout.total, |slot| slot.y);
    if lead > 0. {
        column = column.child(
            rect()
                .key("lead")
                .width(Size::fill())
                .height(Size::px(lead)),
        );
    }

    for (index, slot) in slots[first..last].iter().enumerate() {
        column = column.child(
            rect()
                .key(first + index)
                .width(Size::fill())
                .height(Size::px(slot.height + GRID_GAP))
                .child(
                    rect()
                        .width(Size::fill())
                        .height(Size::px(slot.height))
                        .child(match &slot.content {
                            SlotContent::Fixed(element) => element.clone(),
                            SlotContent::Row(cards) => card_row(ctx, cards, layout.card_width),
                        }),
                ),
        );
    }

    let tail = slots.get(last).map_or(0., |slot| layout.total - slot.y);
    if tail > 0. {
        column = column.child(
            rect()
                .key("tail")
                .width(Size::fill())
                .height(Size::px(tail)),
        );
    }

    column.into_element()
}

struct GridCtx<'a> {
    data: &'a WardrobeData,
    shared: Shared,
}

impl GridCtx<'_> {
    fn grid(&self, cards: Vec<Card>) -> Block {
        Block::Cards(cards)
    }

    fn run(&self, keys: WardrobeAction) -> EventHandler<()> {
        self.shared.run(keys)
    }

    fn select(&self, picked: Selection) -> EventHandler<()> {
        let mut selection = self.shared.selection;
        let mut trying_on = self.shared.trying_on;
        let mut show_cart = self.shared.show_cart;
        let mut adding_skin = self.shared.adding_skin;
        EventHandler::new(move |()| {
            show_cart.set(false);
            adding_skin.set(false);
            trying_on.set(true);
            selection.set(Some(picked.clone()));
        })
    }

    fn equip(&self, toggle: EventHandler<()>) -> EventHandler<()> {
        let mut trying_on = self.shared.trying_on;
        EventHandler::new(move |()| {
            trying_on.set(false);
            toggle.call(());
        })
    }

    fn selected(&self) -> Option<Selection> {
        self.shared.selection.read().clone()
    }

    fn anchor(&self, text: &str) -> Block {
        Block::Fixed {
            element: rect()
                .width(Size::fill())
                .height(Size::px(ANCHOR_HEIGHT))
                .main_align(Alignment::End)
                .child(divider(text))
                .into_element(),
            height: ANCHOR_HEIGHT,
            anchor: Some(text.to_string()),
        }
    }
}

fn equip_strip(equipped: bool) -> (&'static str, IconType) {
    if equipped {
        (t("oneclient.wardrobe.action.equipped"), IconType::Check)
    } else {
        (t("oneclient.wardrobe.action.equip"), IconType::Plus)
    }
}

fn featured_rows(ctx: &GridCtx) -> Vec<Block> {
    let data = ctx.data;
    let Some(store) = &data.store else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (text, items) in [
        (t("oneclient.wardrobe.featured.popular"), &store.popular),
        (t("oneclient.wardrobe.featured.new"), &store.newest),
    ] {
        let cards: Vec<Card> = items
            .iter()
            .filter(|item| data.matches(&item.name))
            .map(|item| Card::Shop(item.id))
            .collect();
        if !cards.is_empty() {
            out.push(ctx.anchor(text));
            out.push(ctx.grid(cards));
        }
    }
    let bundles: Vec<Card> = store
        .bundles
        .iter()
        .filter(|bundle| data.matches(&bundle.name))
        .map(|bundle| Card::Bundle(bundle.id))
        .collect();
    if !bundles.is_empty() {
        out.push(ctx.anchor(t("oneclient.wardrobe.featured.bundles")));
        out.push(ctx.grid(bundles));
    }
    out
}

/// Owned cards, then the store's under a small label so the two never blur together
fn owned_then_shop(owned: Vec<Card>, shop: Vec<Card>) -> Vec<Block> {
    let mut out = Vec::new();
    if !owned.is_empty() {
        out.push(Block::Cards(owned));
    }
    if !shop.is_empty() {
        out.push(Block::fixed(
            store_label(t("oneclient.wardrobe.store.section")),
            STORE_LABEL_HEIGHT,
        ));
        out.push(Block::Cards(shop));
    }
    out
}

fn store_missing(data: &WardrobeData, kind: CosmeticType) -> bool {
    data.store
        .as_ref()
        .is_some_and(|store| store.missing.contains(&kind))
}

fn cosmetics_content(ctx: &GridCtx) -> Vec<Block> {
    let data = ctx.data;
    let mut out = Vec::new();
    let searching = !data.query.trim().is_empty();
    let store_down = data.store.as_ref().is_some_and(|store| store.failed);
    out.extend(featured_rows(ctx));

    for (kind, text) in &categories() {
        if *kind == CosmeticType::Cape {
            let capes = cape_groups(ctx);
            if !capes.is_empty() {
                out.push(ctx.anchor(text));
                out.extend(capes);
            }
            continue;
        }

        let owned = data.groups(*kind);
        let shop = data.shop(*kind);
        let missing = !searching && !store_down && store_missing(data, *kind);
        if owned.is_empty() && shop.is_empty() && !missing {
            continue;
        }

        out.push(ctx.anchor(text));
        out.extend(owned_then_shop(
            owned
                .into_iter()
                .map(|group| Card::Owned(group.id))
                .collect(),
            shop.iter().map(|item| Card::Shop(item.id)).collect(),
        ));
        if missing {
            out.push(Block::notice(t("oneclient.wardrobe.store.unreachable")));
        }
    }

    let emotes = owned_then_shop(
        data.emotes()
            .iter()
            .filter(|emote| data.matches(&emote.name))
            .map(|emote| Card::Emote(emote.id))
            .collect(),
        data.shop(CosmeticType::Emote)
            .iter()
            .map(|item| Card::Shop(item.id))
            .collect(),
    );
    let emotes_missing = !searching && !store_down && store_missing(data, CosmeticType::Emote);
    if !emotes.is_empty() || emotes_missing {
        out.push(ctx.anchor(t("oneclient.wardrobe.category.emotes")));
        out.extend(emotes);
        if emotes_missing {
            out.push(Block::notice(t("oneclient.wardrobe.store.unreachable")));
        }
    }

    if out.is_empty() {
        out.push(Block::notice(t("oneclient.wardrobe.cosmetics.no_match")));
    }
    if store_down {
        out.insert(0, Block::notice(t("oneclient.wardrobe.store.unreachable")));
    }
    out
}

fn cape_groups(ctx: &GridCtx) -> Vec<Block> {
    let data = ctx.data;
    let searching = !data.query.trim().is_empty();
    let mojang_matches = data.look.as_ref().map(|look| {
        look.capes
            .iter()
            .filter(|cape| data.matches(&cape.alias))
            .collect::<Vec<&MojangCape>>()
    });
    let plus_matches = data.plus.as_ref().map(|_| {
        (
            data.groups(CosmeticType::Cape),
            data.shop(CosmeticType::Cape),
        )
    });
    if searching
        && mojang_matches.as_ref().is_some_and(Vec::is_empty)
        && plus_matches
            .as_ref()
            .is_some_and(|(owned, shop)| owned.is_empty() && shop.is_empty())
    {
        return Vec::new();
    }

    let mut out = vec![Block::heading(
        t("oneclient.wardrobe.capes.minecraft.title"),
        t("oneclient.wardrobe.capes.minecraft.description"),
    )];

    match (&data.look, &data.look_error) {
        (None, Some(error)) => out.push(Block::notice(error.clone())),
        (None, None) => out.push(Block::notice(t(
            "oneclient.wardrobe.capes.minecraft.loading",
        ))),
        (Some(look), _) => {
            let capes = mojang_matches.unwrap_or_default();
            if look.capes.is_empty() {
                out.push(Block::notice(t("oneclient.wardrobe.capes.minecraft.empty")));
            } else if capes.is_empty() {
                out.push(Block::notice(t(
                    "oneclient.wardrobe.capes.minecraft.no_match",
                )));
            } else {
                out.push(
                    ctx.grid(
                        capes
                            .into_iter()
                            .map(|cape| Card::Mojang(cape.id.clone()))
                            .collect(),
                    ),
                );
            }
        }
    }

    out.push(Block::heading(
        t("oneclient.wardrobe.capes.plus.title"),
        t("oneclient.wardrobe.capes.plus.description"),
    ));

    let Some((owned, shop)) = plus_matches else {
        out.push(Block::notice(match &data.plus_error {
            Some(error) => error.clone(),
            None => t("oneclient.wardrobe.capes.plus.loading").into(),
        }));
        return out;
    };

    if owned.is_empty() && shop.is_empty() {
        let store_down = data.store.as_ref().is_some_and(|store| store.failed);
        out.push(Block::notice(
            if !store_down && store_missing(data, CosmeticType::Cape) {
                t("oneclient.wardrobe.store.unreachable")
            } else if searching {
                t("oneclient.wardrobe.capes.plus.no_match")
            } else if store_down {
                t("oneclient.wardrobe.capes.plus.empty")
            } else {
                t("oneclient.wardrobe.capes.plus.not_found")
            },
        ));
    } else {
        out.extend(owned_then_shop(
            owned
                .into_iter()
                .map(|group| Card::Owned(group.id))
                .collect(),
            shop.iter().map(|item| Card::Shop(item.id)).collect(),
        ));
    }

    out
}

fn mojang_card(ctx: &GridCtx, cape: &MojangCape) -> Element {
    let data = ctx.data;
    let active = cape.state.eq_ignore_ascii_case("ACTIVE");
    let toggle = ctx.run(toggle_mojang_action(data, cape));
    let (action_label, action_icon) = equip_strip(active);
    let picked = Selection::MojangCape(cape.id.clone());

    WardrobeCard {
        name: cape.alias.clone(),
        detail: detail_text(t("oneclient.wardrobe.minecraft_cape")),
        action_label,
        action_icon,
        equipped: active,
        action_enabled: true,
        selected: ctx.selected() == Some(picked.clone()),
        badge: None,
        thumbnail: LookThumb {
            uuid: data.uuid.clone(),
            skin: None,
            cape: Some(CapeSource::Mojang(cape.url.clone())),
            cosmetics: Vec::new(),
            emote: None,
            particle_color: None,
            yaw: BACK_YAW,
            zoom: CAPE_ZOOM,
            interactive: false,
        }
        .into_element(),
        preview: None,
        menu: vec![{
            let toggle = ctx.equip(toggle.clone());
            MenuAction::new(
                if active {
                    IconType::XClose
                } else {
                    IconType::Plus
                },
                if active {
                    t("oneclient.common.remove")
                } else {
                    t("oneclient.wardrobe.action.equip")
                },
                true,
                move |()| toggle.call(()),
            )
        }],
        rename: None,
        enabled: !data.busy,
        swatches: Vec::new(),
        on_pick: None,
        on_select: ctx.select(picked.clone()),
        on_action: ctx.equip(toggle),
    }
    .into_element()
}

fn owned_card(ctx: &GridCtx, group: &CosmeticGroup) -> Element {
    let data = ctx.data;
    let variant = data.selected_variant(group);
    let worn = data.pick_worn(group);
    let equipped = !data.worn(group).is_empty();
    let (action_label, action_icon) = equip_strip(worn);
    let picked = Selection::Owned(group.id);
    let mut status = ctx.shared.status;

    let toggle = match toggle_owned(data, group) {
        Ok(keys) => ctx.run(keys),
        Err(message) => EventHandler::new(move |()| status.set(Some(message.clone()))),
    };
    let on_equip = ctx.equip(toggle);

    let thumbnail = CoverThumb {
        url: variant.and_then(|variant| data.variant_cover(variant)),
    }
    .into_element();

    // Swatch ids are indexes into the chip list, since a Both chip has no single variant id
    let options = variant_options(group);
    let current = pick_key(group, &data.pick(group));
    let swatches: Vec<Swatch> = options
        .iter()
        .enumerate()
        .map(|(index, (name, pick))| Swatch {
            id: index as i32,
            name: name.clone(),
            texture: pick
                .last()
                .and_then(|(_, id)| data.variant(*id))
                .and_then(|(_, variant)| variant.url.clone()),
            cape: group.kind == CosmeticType::Cape,
            selected: pick_key(group, pick) == current,
        })
        .collect();
    let on_pick = {
        let mut picks = ctx.shared.variant_picks;
        let preview = ctx.select(picked.clone());
        let group_id = group.id;
        EventHandler::new(move |index: i32| {
            if let Some((_, pick)) = usize::try_from(index).ok().and_then(|i| options.get(i)) {
                picks.write().insert(group_id, pick.clone());
            }
            preview.call(());
        })
    };

    WardrobeCard {
        name: group.name.clone(),
        detail: detail_text(collection_label(group.kind)),
        action_label,
        action_icon,
        equipped,
        action_enabled: true,
        selected: ctx.selected() == Some(picked.clone()),
        badge: None,
        thumbnail,
        preview: variant.and_then(|variant| data.preview_for_variant(variant.id)),
        menu: vec![
            {
                let preview = ctx.select(picked.clone());
                MenuAction::new(
                    IconType::Eye,
                    t("oneclient.wardrobe.action.preview"),
                    true,
                    move |()| preview.call(()),
                )
            },
            {
                let on_equip = on_equip.clone();
                MenuAction::new(
                    if worn {
                        IconType::XClose
                    } else {
                        IconType::Plus
                    },
                    if worn {
                        t("oneclient.wardrobe.action.unequip")
                    } else {
                        t("oneclient.wardrobe.action.equip")
                    },
                    !data.busy,
                    move |()| on_equip.call(()),
                )
            },
        ],
        rename: None,
        enabled: true,
        swatches,
        on_pick: Some(on_pick),
        on_select: ctx.select(picked),
        on_action: on_equip,
    }
    .into_element()
}

fn shop_card(ctx: &GridCtx, item: &StoreItem) -> Element {
    let data = ctx.data;
    let variant = data.shop_variant(item);
    let owned_ids = data.owned_ids();
    let owned = data.fully_owned(item) || owned_ids.contains(&variant.id);
    let in_cart = data.cart_keys.contains(&cart_key(variant.id));
    let variants = item.ui_variants();
    let ownership_known = data.plus.is_some();

    let (action_label, action_icon, action_enabled) = if owned {
        (t("oneclient.wardrobe.action.owned"), IconType::Check, false)
    } else if in_cart {
        (
            t("oneclient.wardrobe.action.in_cart"),
            IconType::Check,
            false,
        )
    } else {
        (
            t("oneclient.wardrobe.action.add_to_cart"),
            IconType::Plus,
            ownership_known,
        )
    };

    let badge = if owned {
        None
    } else if item.discounted() {
        Some(Badge::text(
            tf(
                "oneclient.wardrobe.badge.off",
                &[&item.discount_percent().unwrap_or(0)],
            ),
            BadgeStyle::Sale,
        ))
    } else if is_new(&item.created_at) {
        Some(Badge::text(
            t("oneclient.wardrobe.badge.new"),
            BadgeStyle::New,
        ))
    } else {
        None
    };

    let detail = rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(6.)
        .child(price_label(
            item.base_price,
            item.final_price(),
            item.discounted(),
        ))
        .maybe_child((variants.len() > 1).then(|| {
            label()
                .text(tf("oneclient.wardrobe.bundled", &[&variants.len()]))
                .font_size(text_size(11.))
                .color(colors::fg_secondary())
        }))
        .into_element();

    let add = {
        let cart = ctx.shared.cart;
        let status = ctx.shared.status;
        let item = item.clone();
        let variant = variant.clone();
        let already_owned = owned_ids.contains(&variant.id);
        EventHandler::new(move |()| add_to_cart(already_owned, cart, status, &item, &variant))
    };
    let buy = ctx.run(WardrobeAction::BuyNow {
        account: data.account,
        cosmetic: variant.id,
        name: cart_name(item, &variant),
    });
    let preview = ctx.select(Selection::Shop(item.id));

    let swatches: Vec<Swatch> = variants
        .iter()
        .map(|option| Swatch {
            id: option.id,
            name: variant_label(option, item),
            texture: option.asset_id.or(item.asset_id).map(api::asset_url),
            cape: item.kind == CosmeticType::Cape,
            selected: option.id == variant.id,
        })
        .collect();
    let on_pick = {
        let mut picks = ctx.shared.store_picks;
        let item_id = item.id;
        EventHandler::new(move |variant_id: i32| {
            picks.write().insert(item_id, variant_id);
        })
    };

    WardrobeCard {
        name: item.name.clone(),
        detail,
        action_label,
        action_icon,
        equipped: false,
        action_enabled,
        selected: ctx.selected() == Some(Selection::Shop(item.id)),
        badge,
        thumbnail: shop_thumbnail(data, item, &variant, false),
        preview: variant
            .preview_url
            .clone()
            .or_else(|| item.preview_url.clone()),
        menu: vec![
            {
                let preview = preview.clone();
                MenuAction::new(
                    IconType::Eye,
                    t("oneclient.wardrobe.action.preview"),
                    true,
                    move |()| preview.call(()),
                )
            },
            {
                let add = add.clone();
                MenuAction::new(
                    IconType::Plus,
                    t("oneclient.wardrobe.action.add_to_cart"),
                    !in_cart && !owned && ownership_known,
                    move |()| add.call(()),
                )
            },
            MenuAction::new(
                IconType::LinkExternal01,
                t("oneclient.wardrobe.action.buy_now"),
                !owned && ownership_known,
                move |()| buy.call(()),
            ),
        ],
        rename: None,
        enabled: true,
        swatches,
        on_pick: Some(on_pick),
        on_select: preview,
        on_action: add,
    }
    .into_element()
}

fn bundle_card(ctx: &GridCtx, bundle: &BundleInfo) -> Element {
    let data = ctx.data;
    let view = ctx.select(Selection::Bundle(bundle.id));
    let mut status = ctx.shared.status;
    let buy = match bundle.store_product_id.clone() {
        Some(product) => ctx.run(WardrobeAction::Checkout {
            account: data.account,
            products: vec![product],
        }),
        None => {
            let message = tf("oneclient.wardrobe.not_purchasable", &[&bundle.name]);
            EventHandler::new(move |()| status.set(Some(message.clone())))
        }
    };

    WardrobeCard {
        name: bundle.name.clone(),
        detail: price_label(bundle.base_price, bundle.final_price(), bundle.discounted()),
        action_label: t("oneclient.wardrobe.action.view_bundle"),
        action_icon: IconType::ArrowRight,
        equipped: false,
        action_enabled: true,
        selected: ctx.selected() == Some(Selection::Bundle(bundle.id)),
        badge: Some(if bundle.discounted() {
            Badge::text(
                tf(
                    "oneclient.wardrobe.badge.save",
                    &[&bundle.discount_percent().unwrap_or(0)],
                ),
                BadgeStyle::Sale,
            )
        } else {
            Badge::text(t("oneclient.wardrobe.badge.bundle"), BadgeStyle::Accent)
        }),
        thumbnail: CoverThumb {
            url: bundle.asset_id.map(api::asset_url),
        }
        .into_element(),
        preview: None,
        menu: vec![
            {
                let view = view.clone();
                MenuAction::new(
                    IconType::ArrowRight,
                    t("oneclient.wardrobe.action.view_bundle"),
                    true,
                    move |()| view.call(()),
                )
            },
            MenuAction::new(
                IconType::LinkExternal01,
                t("oneclient.wardrobe.action.buy_now"),
                true,
                move |()| buy.call(()),
            ),
        ],
        rename: None,
        enabled: true,
        swatches: Vec::new(),
        on_pick: None,
        on_select: view.clone(),
        on_action: view,
    }
    .into_element()
}

fn emote_card(ctx: &GridCtx, emote: &EmoteInfo) -> Element {
    let data = ctx.data;
    let picked = Selection::Emote(emote.id);

    WardrobeCard {
        name: emote.name.clone(),
        detail: detail_text(collection_label(CosmeticType::Emote)),
        action_label: t("oneclient.wardrobe.action.owned"),
        action_icon: IconType::Check,
        equipped: false,
        action_enabled: false,
        selected: ctx.selected() == Some(picked.clone()),
        badge: None,
        thumbnail: CoverThumb {
            url: data.cover_for_variant(emote.id),
        }
        .into_element(),
        preview: data.preview_for_variant(emote.id),
        menu: vec![{
            let preview = ctx.select(picked.clone());
            MenuAction::new(
                IconType::Eye,
                t("oneclient.wardrobe.action.preview"),
                true,
                move |()| preview.call(()),
            )
        }],
        rename: None,
        enabled: true,
        swatches: Vec::new(),
        on_pick: None,
        on_select: ctx.select(picked),
        on_action: EventHandler::new(|()| {}),
    }
    .into_element()
}

fn outfits_content(ctx: &GridCtx) -> Vec<Block> {
    let data = ctx.data;
    let Some(list) = &data.outfits else {
        return vec![match &data.outfits_error {
            Some(error) => Block::notice(error.clone()),
            None => Block::notice(t("oneclient.wardrobe.outfits.loading")),
        }];
    };

    let mut out = vec![Block::intro(
        tf(
            "oneclient.wardrobe.outfits.intro",
            &[&list.outfits.len(), &list.limit],
        ),
        None,
    )];

    let outfits: Vec<OutfitInfo> = data
        .outfits_in_order()
        .into_iter()
        .filter(|outfit| data.matches(&outfit.name))
        .collect();
    if list.outfits.is_empty() {
        out.push(Block::notice(t("oneclient.wardrobe.outfits.empty")));
        return out;
    }
    if outfits.is_empty() {
        out.push(Block::notice(t("oneclient.wardrobe.outfits.no_match")));
        return out;
    }

    out.push(
        ctx.grid(
            outfits
                .iter()
                .map(|outfit| Card::Outfit(outfit.id))
                .collect(),
        ),
    );
    out
}

fn outfit_card(ctx: &GridCtx, outfit: &OutfitInfo) -> Element {
    let data = ctx.data;
    let active = data.outfits.as_ref().and_then(|list| list.active_outfit_id) == Some(outfit.id);
    let (skin, cape) = data.outfit_look(outfit);
    let id = outfit.id;
    let account = data.account;
    let mut deleting = ctx.shared.deleting;
    let detail = if active {
        t("oneclient.wardrobe.outfits.active").to_string()
    } else {
        cosmetic_count_label(outfit.equipped.len())
    };
    let wear = ctx.run(WardrobeAction::ApplyOutfit {
        account,
        id,
        name: outfit.name.clone(),
    });
    let favorite = ctx.run(WardrobeAction::FavoriteOutfit {
        account,
        id,
        name: outfit.name.clone(),
        favorite: !outfit.favorite,
    });
    let delete = WardrobeAction::DeleteOutfit {
        account,
        id,
        name: outfit.name.clone(),
    };
    let rename = {
        let action = ctx.shared.action;
        let status = ctx.shared.status;
        let old = outfit.name.clone();
        EventHandler::new(move |name: String| {
            perform(
                action,
                status,
                WardrobeAction::RenameOutfit {
                    account,
                    id,
                    old: old.clone(),
                    name,
                },
            )
        })
    };

    WardrobeCard {
        name: outfit.name.clone(),
        detail: detail_text(detail),
        action_label: if active {
            t("oneclient.wardrobe.action.wearing")
        } else {
            t("oneclient.wardrobe.action.wear")
        },
        action_icon: if active {
            IconType::Check
        } else {
            IconType::Plus
        },
        equipped: active,
        action_enabled: !active,
        selected: ctx.selected() == Some(Selection::Outfit(id)),
        badge: outfit.favorite.then(Badge::favorite),
        thumbnail: LookThumb {
            uuid: data.uuid.clone(),
            cosmetics: data.look_cosmetics(&outfit.equipped, Vec::new()),
            emote: None,
            particle_color: outfit.particle_color,
            skin,
            cape,
            yaw: FRONT_YAW,
            zoom: 1.0,
            interactive: false,
        }
        .into_element(),
        preview: None,
        menu: vec![
            {
                let wear = wear.clone();
                MenuAction::new(
                    IconType::Plus,
                    t("oneclient.wardrobe.action.wear"),
                    !active,
                    move |()| wear.call(()),
                )
            },
            MenuAction::new(
                if outfit.favorite {
                    IconType::XClose
                } else {
                    IconType::Check
                },
                if outfit.favorite {
                    t("oneclient.wardrobe.action.unfavourite")
                } else {
                    t("oneclient.wardrobe.action.favourite")
                },
                true,
                move |()| favorite.call(()),
            ),
            {
                let name = outfit.name.clone();
                MenuAction::danger(
                    IconType::Trash01,
                    t("oneclient.common.delete"),
                    true,
                    move |()| {
                        deleting.set(Some((Selection::Outfit(id), name.clone(), delete.clone())));
                    },
                )
            },
        ],
        rename: Some(Rename {
            current: outfit.name.clone(),
            on_save: rename,
        }),
        enabled: !data.busy,
        swatches: Vec::new(),
        on_pick: None,
        on_select: ctx.select(Selection::Outfit(id)),
        on_action: wear,
    }
    .into_element()
}

fn skins_content(ctx: &GridCtx) -> Vec<Block> {
    let data = ctx.data;
    let Some(list) = &data.skins else {
        return vec![match &data.skins_error {
            Some(error) => Block::notice(error.clone()),
            None => Block::notice(t("oneclient.wardrobe.skins.loading")),
        }];
    };

    let mut adding_skin = ctx.shared.adding_skin;
    let mut show_cart = ctx.shared.show_cart;
    let mut status = ctx.shared.status;
    let busy = ctx.data.busy;
    let mut out = vec![Block::intro(
        tf(
            "oneclient.wardrobe.skins.intro",
            &[&list.skins.len(), &list.limit],
        ),
        Some(
            Button::new()
                .secondary()
                .on_press(move |_| {
                    show_cart.set(false);
                    if !busy {
                        status.set(None);
                    }
                    adding_skin.set(true);
                })
                .child(Icon::new(IconType::Plus).size(14.))
                .text(t("oneclient.wardrobe.skins.add"))
                .into_element(),
        ),
    )];

    let skins: Vec<SkinInfo> = favorites_first(&list.skins, |skin| skin.favorite)
        .into_iter()
        .filter(|skin| data.matches(&skin.name))
        .collect();
    if list.skins.is_empty() {
        out.push(Block::notice(t("oneclient.wardrobe.skins.empty")));
        return out;
    }
    if skins.is_empty() {
        out.push(Block::notice(t("oneclient.wardrobe.skins.no_match")));
        return out;
    }

    out.push(ctx.grid(skins.iter().map(|skin| Card::Skin(skin.id)).collect()));
    out
}

fn skin_card(ctx: &GridCtx, skin: &SkinInfo) -> Element {
    let data = ctx.data;
    let id = skin.id;
    let account = data.account;
    let slim = skin.model == SkinModel::Slim;
    let wearing =
        data.current_skin_hash().as_deref() == Some(skin.hash.as_str()) && data.slim == slim;
    let mut deleting = ctx.shared.deleting;
    let wear = ctx.run(WardrobeAction::WearSkin {
        account,
        url: skin.url.clone(),
        slim,
        name: skin.name.clone(),
    });
    let favorite = ctx.run(WardrobeAction::FavoriteSkin {
        account,
        id,
        name: skin.name.clone(),
        favorite: !skin.favorite,
    });
    let arms = ctx.run(WardrobeAction::SkinArms {
        account,
        id,
        url: skin.url.clone(),
        name: skin.name.clone(),
        slim: !slim,
    });
    let delete = WardrobeAction::DeleteSkin {
        account,
        id,
        name: skin.name.clone(),
    };
    let rename = {
        let action = ctx.shared.action;
        let status = ctx.shared.status;
        let old = skin.name.clone();
        EventHandler::new(move |name: String| {
            perform(
                action,
                status,
                WardrobeAction::RenameSkin {
                    account,
                    id,
                    old: old.clone(),
                    name,
                },
            )
        })
    };

    WardrobeCard {
        name: skin.name.clone(),
        detail: detail_text(if slim {
            t("oneclient.wardrobe.skins.slim")
        } else {
            t("oneclient.wardrobe.skins.classic")
        }),
        action_label: if wearing {
            t("oneclient.wardrobe.action.wearing")
        } else {
            t("oneclient.wardrobe.action.wear")
        },
        action_icon: if wearing {
            IconType::Check
        } else {
            IconType::Plus
        },
        equipped: wearing,
        action_enabled: !wearing,
        selected: ctx.selected() == Some(Selection::Skin(id)),
        badge: skin.favorite.then(Badge::favorite),
        thumbnail: LookThumb {
            uuid: data.uuid.clone(),
            skin: Some((skin.url.clone(), slim)),
            cape: None,
            cosmetics: Vec::new(),
            emote: None,
            particle_color: None,
            yaw: FRONT_YAW,
            zoom: 1.0,
            interactive: false,
        }
        .into_element(),
        preview: None,
        menu: vec![
            {
                let wear = wear.clone();
                MenuAction::new(
                    IconType::Plus,
                    t("oneclient.wardrobe.action.wear"),
                    !wearing,
                    move |()| wear.call(()),
                )
            },
            MenuAction::new(
                if skin.favorite {
                    IconType::XClose
                } else {
                    IconType::Check
                },
                if skin.favorite {
                    t("oneclient.wardrobe.action.unfavourite")
                } else {
                    t("oneclient.wardrobe.action.favourite")
                },
                true,
                move |()| favorite.call(()),
            ),
            MenuAction::new(
                IconType::Users01,
                if slim {
                    t("oneclient.wardrobe.skins.switch_classic")
                } else {
                    t("oneclient.wardrobe.skins.switch_slim")
                },
                true,
                move |()| arms.call(()),
            ),
            {
                let name = skin.name.clone();
                MenuAction::danger(
                    IconType::Trash01,
                    t("oneclient.common.delete"),
                    true,
                    move |()| {
                        deleting.set(Some((Selection::Skin(id), name.clone(), delete.clone())));
                    },
                )
            },
        ],
        rename: Some(Rename {
            current: skin.name.clone(),
            on_save: rename,
        }),
        enabled: !data.busy,
        swatches: Vec::new(),
        on_pick: None,
        on_select: ctx.select(Selection::Skin(id)),
        on_action: wear,
    }
    .into_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_columns_stay_between_min_and_max() {
        assert_eq!(grid_columns(0.), (MIN_COLUMNS, CARD_WIDTH));
        assert_eq!(grid_columns(300.).0, MIN_COLUMNS);
        let (columns, width) = grid_columns(5000.);
        assert_eq!(columns, MAX_COLUMNS);
        let used = width * columns as f32 + GRID_GAP * (columns as f32 - 1.);
        assert!((used - 5000.).abs() < 0.01);
    }

    #[test]
    fn glove_variants_equip_to_their_own_hand() {
        let variant = |id, slots: Vec<BodySlot>| CosmeticVariant {
            id,
            name: id.to_string(),
            model: None,
            allowed_slots: slots,
            url: None,
            cover_url: None,
            hash: String::new(),
        };
        let both = vec![BodySlot::LeftHand, BodySlot::RightHand];
        let group = CosmeticGroup {
            id: 1,
            kind: CosmeticType::Glove,
            name: "Glove".into(),
            allowed_slots: both.clone(),
            variants: Vec::new(),
        };
        let slot = |v: &CosmeticVariant| equip_slot(&group, v);
        assert_eq!(
            slot(&variant(1, vec![BodySlot::LeftHand])),
            Some(BodySlot::LeftHand)
        );
        assert_eq!(
            slot(&variant(2, vec![BodySlot::RightHand])),
            Some(BodySlot::RightHand)
        );
        assert_eq!(slot(&variant(3, both)), Some(BodySlot::RightHand));
        assert_eq!(slot(&variant(4, Vec::new())), Some(BodySlot::RightHand));
    }

    #[test]
    fn variant_options_match_poly_plus_chips() {
        let variant = |id, name: &str, slot| CosmeticVariant {
            id,
            name: name.into(),
            model: None,
            allowed_slots: vec![slot],
            url: None,
            cover_url: None,
            hash: String::new(),
        };
        let group = |variants| CosmeticGroup {
            id: 1,
            kind: CosmeticType::Glove,
            name: "Glove".into(),
            allowed_slots: Vec::new(),
            variants,
        };
        let labels = |group: &CosmeticGroup| -> Vec<(String, Pick)> { variant_options(group) };

        let nature = group(vec![
            variant(1, "Right", BodySlot::RightHand),
            variant(2, "Left", BodySlot::LeftHand),
        ]);
        let chips = labels(&nature);
        assert_eq!(chips.len(), 3);
        assert_eq!(chips[0], ("Left".into(), vec![(BodySlot::LeftHand, 2)]));
        assert_eq!(chips[1], ("Right".into(), vec![(BodySlot::RightHand, 1)]));
        assert_eq!(
            chips[2].1,
            vec![(BodySlot::LeftHand, 2), (BodySlot::RightHand, 1)]
        );

        // Same name on both hands stays two chips plus a Both
        let gauntlet = group(vec![
            variant(1, "Silver", BodySlot::RightHand),
            variant(2, "Silver", BodySlot::LeftHand),
            variant(3, "Gold", BodySlot::RightHand),
        ]);
        let chips = labels(&gauntlet);
        let names: Vec<&str> = chips.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names[..2], ["Left Silver", "Right Silver"]);
        assert_eq!(
            chips[2].1,
            vec![(BodySlot::LeftHand, 2), (BodySlot::RightHand, 1)]
        );
        assert_eq!(names[3], "Gold");
        assert_eq!(chips.len(), 4);
    }
}
