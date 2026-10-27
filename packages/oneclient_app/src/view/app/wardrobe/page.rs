use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

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
    AnchorChip, BACK_YAW, Badge, BadgeStyle, CAPE_ZOOM, CARD_CHROME, COLUMNS, CapeSource,
    CoverThumb, FRONT_YAW, GRID_GAP, InfoHeading, LookThumb, MenuAction, Rename, SectionChip,
    Swatch, WardrobeCard, detail_text, divider, library_intro, notice, price_label,
};
use super::preview::{HistoryList, RightColumn};
use crate::components::{
    Button, Dropdown, Icon, IconType, ScrollArea, ScrollAreaCtx, TextInput, centered_spinner,
};
use crate::hooks::wardrobe::{
    MojangLookQuery, OutfitsQuery, PlusCosmeticsQuery, SkinsQuery, StoreData, StoreQuery,
    UseWardrobeAction, WardrobeAction, active_cape, active_skin, invalidate_wardrobe_queries,
    plus_message, use_mojang_look, use_outfits, use_plus_cosmetics, use_skin_library, use_store,
    use_wardrobe_action,
};
use crate::hooks::{
    mutation_is_running, query_error, query_is_loading, settled_or_loading, try_default_account,
    use_current_account,
};
use crate::theme::colors;
use crate::ui::centered_note;
use crate::view::app::cluster::confirm_dialog;

const MAX_WIDTH: f32 = 1480.;
const RIGHT_COLUMN_WIDTH: f32 = 320.;

pub(super) const CATEGORIES: &[(CosmeticType, &str)] = &[
    (CosmeticType::Cape, "Cape"),
    (CosmeticType::Backpack, "Back"),
    (CosmeticType::Glasses, "Glasses"),
    (CosmeticType::Wings, "Wings"),
    (CosmeticType::Glove, "Glove"),
    (CosmeticType::Hat, "Hat"),
    (CosmeticType::Aura, "Aura"),
    (CosmeticType::Boots, "Boots"),
    (CosmeticType::Shoulder, "Shoulder"),
    (CosmeticType::Pet, "Pet"),
];

pub(super) const BACK_VIEW_TYPES: [CosmeticType; 3] = [
    CosmeticType::Cape,
    CosmeticType::Wings,
    CosmeticType::Backpack,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Section {
    Featured,
    Outfits,
    Skins,
    Emotes,
    Cosmetics,
}

const SECTIONS: [(Section, &str, IconType); 5] = [
    (Section::Featured, "Featured", IconType::Rocket02),
    (Section::Outfits, "Outfits", IconType::Users01),
    (Section::Skins, "Skins", IconType::Brush01),
    (Section::Emotes, "Emotes", IconType::Play),
    (Section::Cosmetics, "Cosmetics", IconType::Colors),
];

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
            Self::Popular => "Popular",
            Self::Newest => "Newest",
            Self::PriceLow => "Lowest price",
            Self::PriceHigh => "Highest price",
            Self::Name => "Name",
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
    pub variant_picks: HashMap<i32, i32>,
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

    pub fn equipped_slot(&self, group: &CosmeticGroup) -> Option<(BodySlot, i32)> {
        let plus = self.plus.as_ref()?;
        let mut entries: Vec<(BodySlot, i32)> = plus
            .equipped
            .iter()
            .map(|(slot, id)| (*slot, *id))
            .collect();
        entries.sort_by_key(|(slot, _)| *slot);
        entries
            .into_iter()
            .find(|(_, id)| group.variants.iter().any(|variant| variant.id == *id))
    }

    pub fn selected_variant<'a>(&self, group: &'a CosmeticGroup) -> Option<&'a CosmeticVariant> {
        let wanted = self
            .equipped_slot(group)
            .map(|(_, id)| id)
            .or_else(|| self.variant_picks.get(&group.id).copied());
        wanted
            .and_then(|id| group.variants.iter().find(|variant| variant.id == id))
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
        try_on: Option<(BodySlot, String)>,
    ) -> Vec<(BodySlot, String)> {
        let tried = try_on.as_ref().map(|(slot, _)| *slot);
        let mut cosmetics: Vec<(BodySlot, String)> = equipped
            .iter()
            .filter(|(slot, _)| Some(**slot) != tried)
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
        CosmeticType::Emote => "Emote",
        _ => CATEGORIES
            .iter()
            .find(|(category, _)| *category == kind)
            .map_or("Cosmetic", |(_, label)| label),
    }
}

pub(super) fn collection_label(kind: CosmeticType) -> String {
    format!("{} Collection", type_label(kind))
}

pub(super) fn cosmetic_count_label(count: usize) -> String {
    match count {
        0 => "No cosmetics".into(),
        1 => "1 cosmetic".into(),
        n => format!("{n} cosmetics"),
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
    if item.kind == CosmeticType::Cape
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
            zoom: if interactive { 1.0 } else { CAPE_ZOOM },
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
            cosmetics: data.look_cosmetics(&data.equipped(), try_on),
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
    let allowed = |slot: BodySlot| {
        (variant.allowed_slots.is_empty() && group.allowed_slots.is_empty())
            || variant.allowed_slots.contains(&slot)
            || group.allowed_slots.contains(&slot)
    };
    group
        .kind
        .default_slot()
        .filter(|slot| allowed(*slot))
        .or_else(|| {
            variant
                .allowed_slots
                .iter()
                .chain(group.allowed_slots.iter())
                .copied()
                .find(|slot| *slot != BodySlot::Unknown)
        })
}

fn equip_action(
    data: &WardrobeData,
    group: &CosmeticGroup,
    variant: &CosmeticVariant,
) -> Option<WardrobeAction> {
    equip_slot(group, variant).map(|slot| WardrobeAction::Equip {
        account: data.account,
        slot,
        cosmetic: Some(group.variant_for_skin(variant.id, data.slim)),
        name: group.name.clone(),
    })
}

pub(super) fn toggle_owned(
    data: &WardrobeData,
    group: &CosmeticGroup,
) -> Result<WardrobeAction, String> {
    let Some(variant) = data.selected_variant(group) else {
        return Err(format!("{} has no variant to equip.", group.name));
    };
    let wearing = |id: i32| {
        group
            .variants
            .iter()
            .any(|equipped| equipped.id == id && equipped.name == variant.name)
    };
    match data.equipped_slot(group) {
        Some((slot, id)) if wearing(id) => Ok(WardrobeAction::Equip {
            account: data.account,
            slot,
            cosmetic: None,
            name: group.name.clone(),
        }),
        _ => equip_action(data, group, variant)
            .ok_or_else(|| format!("{} can't be equipped.", group.name)),
    }
}

#[derive(Clone)]
pub(super) struct VariantPick {
    account: Uuid,
    group: CosmeticGroup,
    equipped: Option<(BodySlot, i32)>,
    slim: bool,
}

impl VariantPick {
    pub fn new(data: &WardrobeData, group: &CosmeticGroup) -> Self {
        Self {
            account: data.account,
            group: group.clone(),
            equipped: data.equipped_slot(group),
            slim: data.slim,
        }
    }

    pub fn action(&self, variant_id: i32) -> Option<WardrobeAction> {
        let (slot, equipped) = self.equipped?;
        let resolved = self.group.variant_for_skin(variant_id, self.slim);
        if equipped == resolved {
            return None;
        }
        self.group
            .variants
            .iter()
            .any(|variant| variant.id == variant_id)
            .then(|| WardrobeAction::Equip {
                account: self.account,
                slot,
                cosmetic: Some(resolved),
                name: self.group.name.clone(),
            })
    }
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
        status.set(Some("Wait for the current change to finish.".into()));
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
        status.set(Some(format!("You already own {name}.")));
        return;
    }
    if cart.read().iter().any(|entry| entry.key == key) {
        status.set(Some(format!("{name} is already in your cart.")));
        return;
    }
    status.set(Some(format!("Adding {name} to cart...")));
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
                format!("{name} added to cart.")
            }
            Ok(None) => format!("{name} isn't purchasable yet."),
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
            Some(_) => {
                centered_note("The wardrobe needs a Microsoft account. Switch to one to use it.")
            }
            None if query_is_loading(&current) => centered_spinner("Loading your account..."),
            None => centered_note("Sign in with a Microsoft account to use the wardrobe."),
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
    pub variant_picks: State<HashMap<i32, i32>>,
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

        let section = use_state(|| Section::Featured);
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
            variant_picks: use_state(HashMap::<i32, i32>::new),
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
            return centered_spinner("Loading your wardrobe...");
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
                Section::Featured => featured_content(&ctx),
                Section::Outfits => outfits_content(&ctx),
                Section::Skins => skins_content(&ctx),
                Section::Emotes => emotes_content(&ctx),
                Section::Cosmetics => cosmetics_content(&ctx),
            }
        };
        if !history
            && data.plus.is_some()
            && let Some(error) = &data.plus_error
        {
            blocks.insert(
                0,
                Block::notice(format!(
                    "Couldn't refresh your Poly+ data, so this may be out of date. {error}"
                )),
            );
        }
        let layout = Rc::new(layout(blocks, card_height(data.grid_width)));
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
                format!("Delete \"{name}\"?"),
                "This can't be undone.".to_string(),
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
                    .max_width(Size::px(MAX_WIDTH))
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
                                    .text("Wardrobe")
                                    .width(Size::flex(1.0))
                                    .font_size(24.)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .color(colors::fg_primary()),
                            )
                            .child(
                                TextInput::new(query)
                                    .placeholder("Search...")
                                    .compact()
                                    .width(Size::px(260.))
                                    .leading(
                                        Icon::new(IconType::SearchMd)
                                            .size(14.)
                                            .color(colors::fg_secondary())
                                            .into_element(),
                                    ),
                            ),
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
                                    .width(Size::px(RIGHT_COLUMN_WIDTH))
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

fn section_key(section: Section, history: bool) -> u64 {
    let base = match section {
        Section::Featured => 1,
        Section::Outfits => 2,
        Section::Skins => 3,
        Section::Emotes => 4,
        Section::Cosmetics => 5,
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
            .width(Size::fill())
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
            SECTIONS.to_vec()
        } else {
            vec![SECTIONS[4]]
        };

        let mut chips = rect().horizontal().width(Size::flex(1.0)).spacing(8.);
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
        let top = rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(33.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(chips)
            .maybe_child(store_enabled.then(|| {
                Button::new()
                    .primary()
                    .on_press(move |_| {
                        let open = !*show_cart.read();
                        show_cart.set(open);
                    })
                    .text(format!("Cart ({cart_size})"))
            }));

        let left = if history {
            rect()
                .width(Size::flex(1.0))
                .child(
                    label()
                        .text("Purchase history")
                        .font_size(14.)
                        .font_weight(FontWeight::MEDIUM)
                        .color(colors::fg_primary()),
                )
                .into_element()
        } else {
            rect()
                .width(Size::flex(1.0))
                .child(AnchorRow {
                    labels,
                    anchors,
                    scroll,
                })
                .into_element()
        };

        let sort_labels: Vec<String> = Sort::ALL
            .iter()
            .map(|sort| sort.label().to_string())
            .collect();
        let sort_selected = sort.read().label();
        let mut status = shared.status;
        let bottom = rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::px(33.))
            .content(Content::Flex)
            .cross_align(Alignment::Center)
            .spacing(8.)
            .child(left)
            .maybe_child(
                (store_enabled
                    && !history
                    && matches!(current, Section::Emotes | Section::Cosmetics))
                .then(|| {
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
                        "Back to the wardrobe"
                    } else {
                        "Purchase history"
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
                    .tooltip("Refresh")
                    .enabled(!*refreshing.read())
                    .on_press(move |_| {
                        if *refreshing.peek() {
                            return;
                        }
                        refreshing.set(true);
                        status.set(Some("Refreshing cosmetic data...".into()));
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
                                        format!("Couldn't refresh all cosmetic data. {error}")
                                    }
                                    None => "Cosmetic data refreshed.".into(),
                                },
                            );
                        });
                    })
                    .child(Icon::new(IconType::RefreshCw01).size(14.)),
            );

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
const HEADING_HEIGHT: f32 = 28.;
const NOTICE_HEIGHT: f32 = 57.;
const INTRO_HEIGHT: f32 = 33.;
const OVERSCAN: f32 = 480.;

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
}

fn card_height(grid_width: f32) -> f32 {
    let card_width = if grid_width > 0. {
        (grid_width - GRID_GAP * (COLUMNS as f32 - 1.)) / COLUMNS as f32
    } else {
        180.
    };
    card_width + CARD_CHROME
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

fn card_row(ctx: &GridCtx, cards: &[Card]) -> Element {
    let mut row = rect()
        .horizontal()
        .width(Size::fill())
        .height(Size::fill())
        .content(Content::Flex)
        .spacing(GRID_GAP);
    for card in cards {
        let mut hasher = DefaultHasher::new();
        card.hash(&mut hasher);
        row = row.child(
            rect()
                .key(hasher.finish())
                .width(Size::flex(1.0))
                .height(Size::fill())
                .maybe_child(card_element(ctx, card)),
        );
    }
    for _ in cards.len()..COLUMNS {
        row = row.child(rect().width(Size::flex(1.0)).height(Size::fill()));
    }
    row.into_element()
}

fn layout(blocks: Vec<Block>, card_height: f32) -> Layout {
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
                for chunk in cards.chunks(COLUMNS) {
                    slots.push(Slot {
                        y,
                        height: card_height,
                        content: SlotContent::Row(chunk.to_vec()),
                    });
                    y += card_height + GRID_GAP;
                }
            }
        }
    }
    Layout {
        slots,
        anchors,
        total: y,
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
        column = column.child(rect().width(Size::fill()).height(Size::px(lead)));
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
                            SlotContent::Row(cards) => card_row(ctx, cards),
                        }),
                ),
        );
    }
    let tail = slots.get(last).map_or(0., |slot| layout.total - slot.y);
    if tail > 0. {
        column = column.child(rect().width(Size::fill()).height(Size::px(tail)));
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
        ("Equipped", IconType::Check)
    } else {
        ("Equip", IconType::Plus)
    }
}

const STORE_UNREACHABLE: &str = "Couldn't reach the Poly+ store.";

fn store_notice(
    ctx: &GridCtx,
    loading: &str,
    empty: &str,
    failed: impl Fn(&StoreData) -> bool,
) -> Block {
    let data = ctx.data;
    Block::notice(match (&data.store, &data.store_error) {
        (None, Some(error)) => error.clone(),
        (None, None) => loading.to_string(),
        (Some(store), _) if store.failed || failed(store) => STORE_UNREACHABLE.to_string(),
        _ => empty.to_string(),
    })
}

fn featured_content(ctx: &GridCtx) -> Vec<Block> {
    let data = ctx.data;
    let mut out = Vec::new();
    if let Some(store) = &data.store {
        let popular: Vec<&StoreItem> = store
            .popular
            .iter()
            .filter(|item| data.matches(&item.name))
            .collect();
        let newest: Vec<&StoreItem> = store
            .newest
            .iter()
            .filter(|item| data.matches(&item.name))
            .collect();
        let bundles: Vec<&BundleInfo> = store
            .bundles
            .iter()
            .filter(|bundle| data.matches(&bundle.name))
            .collect();
        if !popular.is_empty() {
            out.push(ctx.anchor("Popular"));
            out.push(
                ctx.grid(
                    popular
                        .into_iter()
                        .map(|item| Card::Shop(item.id))
                        .collect(),
                ),
            );
        }
        if !newest.is_empty() {
            out.push(ctx.anchor("New"));
            out.push(ctx.grid(newest.into_iter().map(|item| Card::Shop(item.id)).collect()));
        }
        if !bundles.is_empty() {
            out.push(ctx.anchor("Bundles"));
            out.push(
                ctx.grid(
                    bundles
                        .into_iter()
                        .map(|bundle| Card::Bundle(bundle.id))
                        .collect(),
                ),
            );
        }
    }
    if out.is_empty() {
        out.push(store_notice(
            ctx,
            "Loading the store...",
            if data.query.trim().is_empty() {
                "Nothing featured right now."
            } else {
                "Nothing featured matches your search."
            },
            |store| store.featured_failed,
        ));
    }
    out
}

fn emotes_content(ctx: &GridCtx) -> Vec<Block> {
    let data = ctx.data;
    let mut out = Vec::new();
    let owned: Vec<&EmoteInfo> = data
        .emotes()
        .iter()
        .filter(|emote| data.matches(&emote.name))
        .collect();
    if !owned.is_empty() {
        out.push(ctx.anchor("Your emotes"));
        out.push(
            ctx.grid(
                owned
                    .into_iter()
                    .map(|emote| Card::Emote(emote.id))
                    .collect(),
            ),
        );
    }

    let mut groups: Vec<(String, Vec<StoreItem>)> = Vec::new();
    for item in data.shop(CosmeticType::Emote) {
        let category = item
            .tags
            .category
            .first()
            .filter(|name| !name.trim().is_empty())
            .map(|name| {
                let name = name.trim();
                let mut chars = name.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default()
            })
            .unwrap_or_else(|| "More emotes".into());
        match groups.iter_mut().find(|(name, _)| *name == category) {
            Some((_, items)) => items.push(item),
            None => groups.push((category, vec![item])),
        }
    }
    for (category, items) in groups {
        out.push(ctx.anchor(&category));
        out.push(ctx.grid(items.iter().map(|item| Card::Shop(item.id)).collect()));
    }

    if out.is_empty() {
        out.push(store_notice(
            ctx,
            "Loading emotes...",
            "No emotes found.",
            |store| store.missing.contains(&CosmeticType::Emote),
        ));
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

    for (kind, text) in CATEGORIES {
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
        let mut cards: Vec<Card> = owned
            .into_iter()
            .map(|group| Card::Owned(group.id))
            .collect();
        cards.extend(shop.iter().map(|item| Card::Shop(item.id)));
        if !cards.is_empty() {
            out.push(ctx.grid(cards));
        }
        if missing {
            out.push(Block::notice(STORE_UNREACHABLE));
        }
    }

    if out.is_empty() {
        out.push(Block::notice("No cosmetics match your search."));
    }
    if store_down {
        out.insert(0, Block::notice(STORE_UNREACHABLE));
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
        "Minecraft capes",
        "Everyone sees this cape, including players without Poly+.",
    )];

    match (&data.look, &data.look_error) {
        (None, Some(error)) => out.push(Block::notice(error.clone())),
        (None, None) => out.push(Block::notice("Loading your Minecraft capes...")),
        (Some(look), _) => {
            let capes = mojang_matches.unwrap_or_default();
            if look.capes.is_empty() {
                out.push(Block::notice("You don't own any Minecraft capes."));
            } else if capes.is_empty() {
                out.push(Block::notice("No Minecraft capes match your search."));
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
        "Poly+ capes",
        "Players with Poly+ see this cape instead of your Minecraft cape.",
    ));

    let Some((owned, shop)) = plus_matches else {
        out.push(Block::notice(match &data.plus_error {
            Some(error) => error.clone(),
            None => "Loading your Poly+ capes...".into(),
        }));
        return out;
    };

    if owned.is_empty() && shop.is_empty() {
        let store_down = data.store.as_ref().is_some_and(|store| store.failed);
        out.push(Block::notice(
            if !store_down && store_missing(data, CosmeticType::Cape) {
                STORE_UNREACHABLE
            } else if searching {
                "No Poly+ capes match your search."
            } else if store_down {
                "You don't own any Poly+ capes."
            } else {
                "No Poly+ capes found."
            },
        ));
    } else {
        let mut cards: Vec<Card> = owned
            .into_iter()
            .map(|group| Card::Owned(group.id))
            .collect();
        cards.extend(shop.iter().map(|item| Card::Shop(item.id)));
        out.push(ctx.grid(cards));
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
        detail: detail_text("Minecraft cape"),
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
        menu: vec![{
            let toggle = ctx.equip(toggle.clone());
            MenuAction::new(
                if active {
                    IconType::XClose
                } else {
                    IconType::Plus
                },
                if active { "Remove" } else { "Equip" },
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
    let equipped = data.equipped_slot(group).is_some();
    let (action_label, action_icon) = equip_strip(equipped);
    let picked = Selection::Owned(group.id);
    let mut status = ctx.shared.status;

    let toggle = match toggle_owned(data, group) {
        Ok(keys) => ctx.run(keys),
        Err(message) => EventHandler::new(move |()| status.set(Some(message.clone()))),
    };
    let on_equip = ctx.equip(toggle);

    let thumbnail = match variant {
        Some(variant) if group.kind == CosmeticType::Cape => LookThumb {
            uuid: data.uuid.clone(),
            skin: None,
            cape: variant.url.clone().map(CapeSource::Plus),
            cosmetics: Vec::new(),
            emote: None,
            particle_color: None,
            yaw: BACK_YAW,
            zoom: CAPE_ZOOM,
            interactive: false,
        }
        .into_element(),
        Some(variant) => CoverThumb {
            url: data.variant_cover(variant),
        }
        .into_element(),
        None => CoverThumb { url: None }.into_element(),
    };

    let swatches: Vec<Swatch> = group
        .ui_variants()
        .into_iter()
        .map(|option| Swatch {
            id: option.id,
            name: option.name.clone(),
            texture: option.url.clone(),
            cape: group.kind == CosmeticType::Cape,
            selected: variant.is_some_and(|variant| variant.name == option.name),
        })
        .collect();
    let on_pick = {
        let mut picks = ctx.shared.variant_picks;
        let action = ctx.shared.action;
        let status = ctx.shared.status;
        let group_id = group.id;
        let pick = VariantPick::new(data, group);
        EventHandler::new(move |variant_id: i32| {
            picks.write().insert(group_id, variant_id);
            if let Some(keys) = pick.action(variant_id) {
                perform(action, status, keys);
            }
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
        menu: vec![
            {
                let preview = ctx.select(picked.clone());
                MenuAction::new(IconType::Eye, "Preview", true, move |()| preview.call(()))
            },
            {
                let on_equip = on_equip.clone();
                MenuAction::new(
                    if equipped {
                        IconType::XClose
                    } else {
                        IconType::Plus
                    },
                    if equipped { "Unequip" } else { "Equip" },
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
        ("Owned", IconType::Check, false)
    } else if in_cart {
        ("In cart", IconType::Check, false)
    } else {
        ("Add to cart", IconType::Plus, ownership_known)
    };

    let badge = if owned {
        None
    } else if item.discounted() {
        Some(Badge::text(
            format!("{}% OFF", item.discount_percent().unwrap_or(0)),
            BadgeStyle::Sale,
        ))
    } else if is_new(&item.created_at) {
        Some(Badge::text("NEW", BadgeStyle::New))
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
                .text(format!("{} variants", variants.len()))
                .font_size(11.)
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
        menu: vec![
            {
                let preview = preview.clone();
                MenuAction::new(IconType::Eye, "Preview", true, move |()| preview.call(()))
            },
            {
                let add = add.clone();
                MenuAction::new(
                    IconType::Plus,
                    "Add to cart",
                    !in_cart && !owned && ownership_known,
                    move |()| add.call(()),
                )
            },
            MenuAction::new(
                IconType::LinkExternal01,
                "Buy now",
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
            let message = format!("{} isn't purchasable yet.", bundle.name);
            EventHandler::new(move |()| status.set(Some(message.clone())))
        }
    };

    WardrobeCard {
        name: bundle.name.clone(),
        detail: price_label(bundle.base_price, bundle.final_price(), bundle.discounted()),
        action_label: "View bundle",
        action_icon: IconType::ArrowRight,
        equipped: false,
        action_enabled: true,
        selected: ctx.selected() == Some(Selection::Bundle(bundle.id)),
        badge: Some(if bundle.discounted() {
            Badge::text(
                format!("SAVE {}%", bundle.discount_percent().unwrap_or(0)),
                BadgeStyle::Sale,
            )
        } else {
            Badge::text("BUNDLE", BadgeStyle::Accent)
        }),
        thumbnail: CoverThumb {
            url: bundle.asset_id.map(api::asset_url),
        }
        .into_element(),
        menu: vec![
            {
                let view = view.clone();
                MenuAction::new(IconType::ArrowRight, "View bundle", true, move |()| {
                    view.call(())
                })
            },
            MenuAction::new(IconType::LinkExternal01, "Buy now", true, move |()| {
                buy.call(())
            }),
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
        action_label: "Owned",
        action_icon: IconType::Check,
        equipped: false,
        action_enabled: false,
        selected: ctx.selected() == Some(picked.clone()),
        badge: None,
        thumbnail: CoverThumb {
            url: data.cover_for_variant(emote.id),
        }
        .into_element(),
        menu: vec![{
            let preview = ctx.select(picked.clone());
            MenuAction::new(IconType::Eye, "Preview", true, move |()| preview.call(()))
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
            None => Block::notice("Loading your outfits..."),
        }];
    };

    let mut out = vec![Block::intro(
        format!(
            "{} of {} saved. Save your look and switch between outfits.",
            list.outfits.len(),
            list.limit
        ),
        None,
    )];

    let outfits: Vec<OutfitInfo> = data
        .outfits_in_order()
        .into_iter()
        .filter(|outfit| data.matches(&outfit.name))
        .collect();
    if list.outfits.is_empty() {
        out.push(Block::notice(
            "You have no saved outfits yet. Use + above the preview to save your current look.",
        ));
        return out;
    }
    if outfits.is_empty() {
        out.push(Block::notice("No outfits match your search."));
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
        "Active".to_string()
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
        action_label: if active { "Wearing" } else { "Wear" },
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
            cosmetics: data.look_cosmetics(&outfit.equipped, None),
            emote: None,
            particle_color: outfit.particle_color,
            skin,
            cape,
            yaw: FRONT_YAW,
            zoom: 1.0,
            interactive: false,
        }
        .into_element(),
        menu: vec![
            {
                let wear = wear.clone();
                MenuAction::new(IconType::Plus, "Wear", !active, move |()| wear.call(()))
            },
            MenuAction::new(
                if outfit.favorite {
                    IconType::XClose
                } else {
                    IconType::Check
                },
                if outfit.favorite {
                    "Remove from favourites"
                } else {
                    "Add to favourites"
                },
                true,
                move |()| favorite.call(()),
            ),
            {
                let name = outfit.name.clone();
                MenuAction::danger(IconType::Trash01, "Delete", true, move |()| {
                    deleting.set(Some((Selection::Outfit(id), name.clone(), delete.clone())));
                })
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
            None => Block::notice("Loading your skins..."),
        }];
    };

    let mut adding_skin = ctx.shared.adding_skin;
    let mut show_cart = ctx.shared.show_cart;
    let mut status = ctx.shared.status;
    let busy = ctx.data.busy;
    let mut out = vec![Block::intro(
        format!(
            "{} of {} saved. Every skin you wear is saved here automatically.",
            list.skins.len(),
            list.limit
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
                .text("Add skin")
                .into_element(),
        ),
    )];

    let skins: Vec<SkinInfo> = favorites_first(&list.skins, |skin| skin.favorite)
        .into_iter()
        .filter(|skin| data.matches(&skin.name))
        .collect();
    if list.skins.is_empty() {
        out.push(Block::notice(
            "You have no saved skins yet. Use Add skin to save one.",
        ));
        return out;
    }
    if skins.is_empty() {
        out.push(Block::notice("No skins match your search."));
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
        detail: detail_text(if slim { "Slim" } else { "Classic" }),
        action_label: if wearing { "Wearing" } else { "Wear" },
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
        menu: vec![
            {
                let wear = wear.clone();
                MenuAction::new(IconType::Plus, "Wear", !wearing, move |()| wear.call(()))
            },
            MenuAction::new(
                if skin.favorite {
                    IconType::XClose
                } else {
                    IconType::Check
                },
                if skin.favorite {
                    "Remove from favourites"
                } else {
                    "Add to favourites"
                },
                true,
                move |()| favorite.call(()),
            ),
            MenuAction::new(
                IconType::Users01,
                if slim {
                    "Switch to classic arms"
                } else {
                    "Switch to slim arms"
                },
                true,
                move |()| arms.call(()),
            ),
            {
                let name = skin.name.clone();
                MenuAction::danger(IconType::Trash01, "Delete", true, move |()| {
                    deleting.set(Some((Selection::Skin(id), name.clone(), delete.clone())));
                })
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
