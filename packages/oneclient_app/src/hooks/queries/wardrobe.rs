use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use freya::query::{
    Mutation, MutationCapability, QueriesStorage, Query, QueryCapability, UseMutation, UseQuery,
    use_mutation, use_query,
};
use oneclient_auth::MinecraftAccount;
use oneclient_mc::{MojangFullPlayerProfile, SkinVariant};
use oneclient_polyplus::PlusError;
use oneclient_polyplus::api::{
    self, BodySlot, BundleInfo, CosmeticType, OutfitList, PlayerCosmetics, SkinList, SkinModel,
    StoreItem, StoreSort, Transaction,
};
use uuid::Uuid;

use super::player_profile::FetchPlayerProfileQuery;

const WARDROBE_STALE: Duration = Duration::from_secs(60);
const PROFILE_TTL: Duration = Duration::from_secs(120);

static PROFILES: std::sync::LazyLock<
    std::sync::Mutex<HashMap<Uuid, (MojangFullPlayerProfile, std::time::Instant)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

const RECORD_RETRY: Duration = Duration::from_secs(10 * 60);

type RecordKey = (Uuid, String);

#[derive(Default)]
struct RecordedSkins {
    saved: HashMap<RecordKey, Option<i32>>,
    failed: HashMap<RecordKey, (std::time::Instant, bool)>,
}

impl RecordedSkins {
    fn backing_off(&self, key: &RecordKey) -> bool {
        self.failed
            .get(key)
            .is_some_and(|(at, _)| at.elapsed() < RECORD_RETRY)
    }

    fn blocked(&self, key: &RecordKey) -> bool {
        self.saved.contains_key(key) || self.backing_off(key)
    }

    fn recordable(&self, key: &RecordKey) -> bool {
        match self.saved.get(key) {
            Some(id) => id.is_some(),
            None => !self.backing_off(key),
        }
    }

    fn saved(&mut self, key: RecordKey, id: i32) {
        self.failed.remove(&key);
        self.saved.insert(key, Some(id));
    }

    fn failed(&mut self, key: RecordKey, err: &PlusError) {
        let full = matches!(err, PlusError::Api { status: 409, .. });
        self.failed.insert(key, (std::time::Instant::now(), full));
    }
}

static RECORDED: std::sync::LazyLock<tokio::sync::Mutex<RecordedSkins>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(RecordedSkins::default()));

fn cached_profile(account: Uuid) -> Option<MojangFullPlayerProfile> {
    let profiles = PROFILES.lock().ok()?;
    let (profile, at) = profiles.get(&account)?;
    (at.elapsed() < PROFILE_TTL).then(|| profile.clone())
}

fn cache_profile(account: Uuid, profile: &MojangFullPlayerProfile) {
    if let Ok(mut profiles) = PROFILES.lock() {
        profiles.insert(account, (profile.clone(), std::time::Instant::now()));
    }
}
const TEXTURE_STALE: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WardrobeKeys {
    pub account: Uuid,
}

pub fn plus_message(err: PlusError) -> String {
    match err {
        PlusError::ConsentRequired => {
            "Poly+ needs the Terms of Service and Privacy Policy. Accept them in OneClient's settings."
                .into()
        }
        PlusError::OfflineAccount => "Sign in with a Microsoft account to use Poly+.".into(),
        PlusError::JoinRejected { status: 429, .. } => {
            "Minecraft is limiting sign-ins right now. Try again in a minute.".into()
        }
        PlusError::JoinRejected { status, .. } if status >= 500 => {
            "Minecraft's servers are having trouble. Try again later.".into()
        }
        PlusError::JoinRejected { .. } => {
            "Minecraft didn't accept the sign-in. Sign in to the account again.".into()
        }
        PlusError::LoginRejected { status, .. } if status >= 500 => {
            "The Poly+ servers are having trouble. Try again later.".into()
        }
        PlusError::LoginRejected { .. } => "Poly+ couldn't sign you in. Try again later.".into(),
        PlusError::Api { status: 404, .. } => "That item no longer exists.".into(),
        PlusError::Api { status: 409, .. } => {
            "You've reached the limit. Delete one to make room.".into()
        }
        PlusError::Api { status: 429, .. } => {
            "Too many changes at once. Try again in a moment.".into()
        }
        PlusError::Api { status, .. } if status >= 500 => {
            "The Poly+ servers are having trouble. Try again later.".into()
        }
        PlusError::Api { body, .. } if !body.trim().is_empty() => body.trim().to_string(),
        PlusError::Request(err) if err.is_decode() => {
            "The Poly+ servers sent an unexpected response.".into()
        }
        PlusError::Request(_) | PlusError::Websocket(_) => {
            "Couldn't reach the Poly+ servers.".into()
        }
        other => other.to_string(),
    }
}

fn list_message(err: PlusError, what: &str) -> String {
    match err {
        PlusError::Api { status: 404, .. } => {
            format!("{what} aren't available on the Poly+ servers yet.")
        }
        other => plus_message(other),
    }
}

fn mojang_reason(err: &oneclient_mc::McError) -> Option<String> {
    let oneclient_mc::McError::MinecraftStatus { body, .. } = err else {
        return None;
    };
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get("errorMessage")?
        .as_str()
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string)
}

fn mojang_message(err: oneclient_mc::McError) -> String {
    match err.http_status() {
        Some(429) => "Minecraft is limiting changes right now. Try again in a minute.".into(),
        Some(401 | 403) => {
            "Minecraft didn't accept the sign-in. Sign in to the account again.".into()
        }
        Some(status) if status >= 500 => {
            "Minecraft's servers are having trouble. Try again later.".into()
        }
        Some(status) => match mojang_reason(&err) {
            Some(reason) => format!("Minecraft rejected the change: {reason}"),
            None => format!("Minecraft rejected the change (HTTP {status})."),
        },
        None => format!("Couldn't reach Minecraft's servers ({err})."),
    }
}

fn mojang_read_message(err: oneclient_mc::McError) -> String {
    match err.http_status() {
        Some(404) => "This account doesn't have a Minecraft: Java Edition profile.".into(),
        Some(status) if (400..500).contains(&status) && !matches!(status, 401 | 403 | 429) => {
            format!("Minecraft couldn't load the profile (HTTP {status}).")
        }
        _ => mojang_message(err),
    }
}

async fn fresh_account(id: Uuid) -> Result<MinecraftAccount, String> {
    let state = crate::launcher::state().map_err(|err| err.to_string())?;
    let account = state
        .auth
        .account_for_launch(id)
        .await
        .map_err(|err| err.to_string())?;
    if !account.is_microsoft() {
        return Err("Sign in with a Microsoft account to use the wardrobe.".into());
    }
    if account.needs_sign_in() || account.access_token.is_empty() {
        return Err("Sign in to the account again to use the wardrobe.".into());
    }
    Ok(account)
}

fn forget_profile(account: Uuid) {
    if let Ok(mut profiles) = PROFILES.lock() {
        profiles.remove(&account);
    }
}

async fn mojang_profile(account: &MinecraftAccount) -> Result<MojangFullPlayerProfile, String> {
    if let Some(profile) = cached_profile(account.id) {
        return Ok(profile);
    }
    let state = crate::launcher::state().map_err(|err| err.to_string())?;
    let profile = oneclient_mc::fetch_logged_in_profile_once(
        &state.services.requester,
        &account.access_token,
    )
    .await
    .map_err(mojang_read_message)?;
    cache_profile(account.id, &profile);
    Ok(profile)
}

async fn fresh_profile(account: &MinecraftAccount) -> Result<MojangFullPlayerProfile, String> {
    forget_profile(account.id);
    mojang_profile(account).await
}

fn mojang_write_failed(account: Uuid) -> impl FnOnce(oneclient_mc::McError) -> String {
    move |err| {
        forget_profile(account);
        mojang_message(err)
    }
}

pub fn active_skin(profile: &MojangFullPlayerProfile) -> Option<&oneclient_mc::MojangSkin> {
    profile
        .skins
        .iter()
        .find(|skin| skin.state.eq_ignore_ascii_case("ACTIVE"))
}

pub fn active_cape(profile: &MojangFullPlayerProfile) -> Option<&oneclient_mc::MojangCape> {
    profile
        .capes
        .iter()
        .find(|cape| cape.state.eq_ignore_ascii_case("ACTIVE"))
}

fn skin_model(variant: &SkinVariant) -> SkinModel {
    match variant {
        SkinVariant::Classic => SkinModel::Classic,
        SkinVariant::Slim => SkinModel::Slim,
    }
}

async fn record_current_skin(
    account: &MinecraftAccount,
    profile: &MojangFullPlayerProfile,
    force: bool,
) -> Result<Option<i32>, String> {
    let Some(skin) = active_skin(profile) else {
        return Ok(None);
    };
    let Some(hash) = api::texture_hash(&skin.url) else {
        return Err("Couldn't read your current Minecraft skin.".into());
    };
    let key = (account.id, hash);
    let mut recorded = RECORDED.lock().await;
    if !force {
        if let Some(id) = recorded.saved.get(&key) {
            return id
                .map(Some)
                .ok_or_else(|| "Your current skin was removed from your library.".to_string());
        }
        if recorded.blocked(&key) {
            return Err("Your current skin couldn't be saved to your library recently.".into());
        }
    }
    match add_worn_skin(account, &key.1, skin_model(&skin.variant), None).await {
        Ok(info) => {
            recorded.saved(key, info.id);
            Ok(Some(info.id))
        }
        Err(err) => {
            tracing::warn!("[wardrobe] could not record the current skin: {err}");
            recorded.failed(key, &err);
            Err(plus_message(err))
        }
    }
}

async fn add_worn_skin(
    account: &MinecraftAccount,
    hash: &str,
    model: SkinModel,
    name: Option<&str>,
) -> Result<api::SkinInfo, PlusError> {
    let info = api::add_skin(account, hash, model, name).await?;
    if info.model == model {
        return Ok(info);
    }
    match api::update_skin(account, info.id, &serde_json::json!({ "model": model })).await {
        Ok(updated) => Ok(updated),
        Err(err) => {
            tracing::warn!(
                "[wardrobe] saved skin {} but couldn't change its model: {err}",
                info.id
            );
            Ok(info)
        }
    }
}

fn worn_hash(profile: &MojangFullPlayerProfile) -> Option<String> {
    active_skin(profile).and_then(|skin| api::texture_hash(&skin.url))
}

async fn can_adopt(account: Uuid, profile: &MojangFullPlayerProfile) -> bool {
    match worn_hash(profile) {
        Some(hash) => RECORDED.lock().await.recordable(&(account, hash)),
        None => true,
    }
}

async fn worth_adopting(account: Uuid) -> bool {
    match cached_profile(account) {
        Some(profile) => can_adopt(account, &profile).await,
        None => true,
    }
}

async fn already_recorded(account: Uuid, profile: &MojangFullPlayerProfile) -> bool {
    let Some(hash) = worn_hash(profile) else {
        return true;
    };
    RECORDED.lock().await.blocked(&(account, hash))
}

async fn forget_recorded(account: Uuid, skin: i32) {
    let mut recorded = RECORDED.lock().await;
    for (key, id) in recorded.saved.iter_mut() {
        if key.0 == account && *id == Some(skin) {
            *id = None;
        }
    }
    recorded
        .failed
        .retain(|key, (_, full)| key.0 != account || !*full);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MojangLookQuery;

impl QueryCapability for MojangLookQuery {
    type Ok = MojangFullPlayerProfile;
    type Err = String;
    type Keys = WardrobeKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let account = fresh_account(keys.account).await?;
        mojang_profile(&account).await
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlusCosmeticsQuery;

impl QueryCapability for PlusCosmeticsQuery {
    type Ok = PlayerCosmetics;
    type Err = String;
    type Keys = WardrobeKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let account = fresh_account(keys.account).await?;
        api::fetch_player(&account).await.map_err(plus_message)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OutfitsQuery;

impl QueryCapability for OutfitsQuery {
    type Ok = OutfitList;
    type Err = String;
    type Keys = WardrobeKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let account = fresh_account(keys.account).await?;
        let list = api::list_outfits(&account)
            .await
            .map_err(|err| list_message(err, "Outfits"))?;

        let unadopted = list
            .active_outfit_id
            .and_then(|id| list.outfits.iter().find(|outfit| outfit.id == id))
            .filter(|outfit| outfit.blank)
            .map(|outfit| outfit.id);
        if let Some(id) = unadopted
            && worth_adopting(account.id).await
            && let Ok(profile) = fresh_profile(&account).await
            && can_adopt(account.id, &profile).await
            && adopt_current_look(&account, &profile, id, false).await == Adoption::Saved
        {
            return Ok(api::list_outfits(&account).await.unwrap_or(list));
        }

        Ok(list)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SkinsQuery;

impl QueryCapability for SkinsQuery {
    type Ok = SkinList;
    type Err = String;
    type Keys = WardrobeKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let account = fresh_account(keys.account).await?;
        let list = api::list_skins(&account)
            .await
            .map_err(|err| list_message(err, "Skins"))?;
        if let Ok(profile) = mojang_profile(&account).await
            && !already_recorded(account.id, &profile).await
            && matches!(
                record_current_skin(&account, &profile, false).await,
                Ok(Some(_))
            )
        {
            return Ok(api::list_skins(&account).await.unwrap_or(list));
        }
        Ok(list)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CosmeticBundleKeys {
    pub url: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CosmeticBundle {
    pub geometry: Option<Bytes>,
    pub animation: Option<Bytes>,
    pub texture: Option<Bytes>,
    pub sheet: Option<Bytes>,
    pub pet: Option<Bytes>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CosmeticBundleQuery;

impl QueryCapability for CosmeticBundleQuery {
    type Ok = CosmeticBundle;
    type Err = String;
    type Keys = CosmeticBundleKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let Some(url) = keys.url.as_deref() else {
            return Err("No cosmetic".into());
        };
        let files = api::bundle_files(url).await.map_err(plus_message)?;
        let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
        let parts = plusrenderer::bundle::parts(&names);
        let pick = |index: Option<usize>| index.map(|index| files[index].1.clone());
        Ok(CosmeticBundle {
            geometry: pick(parts.geometry),
            animation: pick(parts.animation),
            texture: pick(parts.texture),
            sheet: pick(parts.sheet),
            pet: pick(names.iter().position(|name| *name == "pet.json")),
        })
    }
}

fn wardrobe_query<Q>(account: Uuid, capability: Q) -> UseQuery<Q>
where
    Q: QueryCapability<Keys = WardrobeKeys>,
{
    use_query(Query::new(WardrobeKeys { account }, capability).stale_time(WARDROBE_STALE))
}

const STORE_TYPES: [CosmeticType; 11] = [
    CosmeticType::Cape,
    CosmeticType::Backpack,
    CosmeticType::Glasses,
    CosmeticType::Wings,
    CosmeticType::Glove,
    CosmeticType::Hat,
    CosmeticType::Aura,
    CosmeticType::Boots,
    CosmeticType::Shoulder,
    CosmeticType::Pet,
    CosmeticType::Emote,
];
const FEATURED_ROW_SIZE: u32 = 6;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct StoreData {
    pub by_type: Vec<(CosmeticType, Vec<StoreItem>)>,
    pub popular: Vec<StoreItem>,
    pub newest: Vec<StoreItem>,
    pub bundles: Vec<BundleInfo>,
    pub failed: bool,
    pub missing: Vec<CosmeticType>,
    pub featured_failed: bool,
}

impl StoreData {
    pub fn of_type(&self, kind: CosmeticType) -> &[StoreItem] {
        self.by_type
            .iter()
            .find(|(entry, _)| *entry == kind)
            .map_or(&[], |(_, items)| items.as_slice())
    }

    pub fn find(&self, id: i32) -> Option<&StoreItem> {
        self.by_type
            .iter()
            .flat_map(|(_, items)| items.iter())
            .chain(self.popular.iter())
            .chain(self.newest.iter())
            .find(|item| item.id == id)
    }

    pub fn containing_variant(&self, variant: i32) -> Option<&StoreItem> {
        self.by_type
            .iter()
            .flat_map(|(_, items)| items.iter())
            .chain(self.popular.iter())
            .chain(self.newest.iter())
            .find(|item| match item.variants.as_deref() {
                Some(variants) if !variants.is_empty() => {
                    variants.iter().any(|entry| entry.id == variant)
                }
                _ => item.id == variant,
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StoreKeys;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StoreQuery;

impl QueryCapability for StoreQuery {
    type Ok = StoreData;
    type Err = String;
    type Keys = StoreKeys;

    async fn run(&self, _keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let per_type = futures_util::future::join_all(STORE_TYPES.iter().map(|kind| async move {
            (
                *kind,
                api::search_store(StoreSort::Popularity, Some(*kind), 100).await,
            )
        }))
        .await;
        let (popular, newest, bundles) = futures_util::future::join3(
            api::search_store(StoreSort::Popularity, None, FEATURED_ROW_SIZE),
            api::search_store(StoreSort::Newest, None, FEATURED_ROW_SIZE),
            api::search_bundles(FEATURED_ROW_SIZE),
        )
        .await;

        if let Some((_, Err(PlusError::ConsentRequired))) = per_type.first() {
            return Err(plus_message(PlusError::ConsentRequired));
        }
        let failed = per_type.iter().all(|(_, result)| result.is_err());
        let missing = per_type
            .iter()
            .filter(|(_, result)| result.is_err())
            .map(|(kind, _)| *kind)
            .collect();
        let featured_failed = popular.is_err() || newest.is_err() || bundles.is_err();
        Ok(StoreData {
            by_type: per_type
                .into_iter()
                .filter_map(|(kind, result)| result.ok().map(|items| (kind, items)))
                .collect(),
            popular: popular.unwrap_or_default(),
            newest: newest.unwrap_or_default(),
            bundles: bundles
                .unwrap_or_default()
                .into_iter()
                .filter(|bundle| {
                    bundle
                        .store_product_id
                        .as_deref()
                        .is_some_and(|id| !id.trim().is_empty())
                })
                .collect(),
            failed,
            missing,
            featured_failed,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransactionsQuery;

impl QueryCapability for TransactionsQuery {
    type Ok = Vec<Transaction>;
    type Err = String;
    type Keys = WardrobeKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let account = fresh_account(keys.account).await?;
        api::transactions(&account)
            .await
            .map_err(|err| format!("Couldn't load transactions: {}", plus_message(err)))
    }
}

pub fn use_store() -> UseQuery<StoreQuery> {
    use_query(Query::new(StoreKeys, StoreQuery).stale_time(Duration::from_secs(5 * 60)))
}

pub fn use_transactions(account: Uuid) -> UseQuery<TransactionsQuery> {
    wardrobe_query(account, TransactionsQuery)
}

pub fn use_mojang_look(account: Uuid) -> UseQuery<MojangLookQuery> {
    wardrobe_query(account, MojangLookQuery)
}

pub fn use_plus_cosmetics(account: Uuid) -> UseQuery<PlusCosmeticsQuery> {
    wardrobe_query(account, PlusCosmeticsQuery)
}

pub fn use_outfits(account: Uuid) -> UseQuery<OutfitsQuery> {
    wardrobe_query(account, OutfitsQuery)
}

pub fn use_skin_library(account: Uuid) -> UseQuery<SkinsQuery> {
    wardrobe_query(account, SkinsQuery)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SwatchKeys {
    pub url: Option<String>,
    pub cape: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SwatchQuery;

impl QueryCapability for SwatchQuery {
    type Ok = api::SwatchArt;
    type Err = String;
    type Keys = SwatchKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let Some(url) = keys.url.as_deref() else {
            return Err("No texture".into());
        };
        api::swatch_art(url, keys.cape)
            .await
            .map_err(|err| err.to_string())
    }
}

pub fn use_swatch(url: Option<String>, cape: bool) -> UseQuery<SwatchQuery> {
    use_query(Query::new(SwatchKeys { url, cape }, SwatchQuery).stale_time(TEXTURE_STALE))
}

pub fn use_cosmetic_bundle(url: Option<String>) -> UseQuery<CosmeticBundleQuery> {
    use_query(Query::new(CosmeticBundleKeys { url }, CosmeticBundleQuery).stale_time(TEXTURE_STALE))
}

async fn invalidate_after(action: &WardrobeAction) {
    let (look, plus, outfits, skins, transactions) = match action {
        WardrobeAction::Equip { .. } | WardrobeAction::ParticleColor { .. } => {
            (false, true, true, false, false)
        }
        WardrobeAction::MojangCape { .. } => (true, false, true, false, false),
        WardrobeAction::WearSkin { .. }
        | WardrobeAction::AddSkinFile { .. }
        | WardrobeAction::AddSkinUrl { .. } => (true, false, true, true, false),
        WardrobeAction::SkinArms { .. } => (true, false, false, true, false),
        WardrobeAction::AddSkinUsername { .. }
        | WardrobeAction::RenameSkin { .. }
        | WardrobeAction::FavoriteSkin { .. } => (false, false, false, true, false),
        WardrobeAction::DeleteSkin { .. } => (false, false, true, true, false),
        WardrobeAction::ApplyOutfit { .. } => (true, true, true, true, false),
        WardrobeAction::CreateOutfit { .. }
        | WardrobeAction::RenameOutfit { .. }
        | WardrobeAction::FavoriteOutfit { .. }
        | WardrobeAction::DeleteOutfit { .. } => (false, false, true, true, false),
        WardrobeAction::Checkout { .. } | WardrobeAction::BuyNow { .. } => {
            (false, true, false, false, true)
        }
    };
    if look {
        QueriesStorage::<MojangLookQuery>::invalidate_all().await;
        QueriesStorage::<FetchPlayerProfileQuery>::invalidate_all().await;
    }
    if plus {
        QueriesStorage::<PlusCosmeticsQuery>::invalidate_all().await;
    }
    if outfits {
        QueriesStorage::<OutfitsQuery>::invalidate_all().await;
    }
    if skins {
        QueriesStorage::<SkinsQuery>::invalidate_all().await;
    }
    if transactions {
        QueriesStorage::<TransactionsQuery>::invalidate_all().await;
    }
}

struct PurchaseWatch {
    begun: std::time::Instant,
    pending: u32,
}

static WATCHING: std::sync::LazyLock<std::sync::Mutex<HashMap<Uuid, PurchaseWatch>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

const WATCH_INTERVAL: Duration = Duration::from_secs(4);
const WATCH_LIMIT: Duration = Duration::from_secs(15 * 60);

fn owned_ids(player: &PlayerCosmetics) -> std::collections::HashSet<(bool, i32)> {
    player
        .cosmetics
        .iter()
        .flat_map(|group| group.variants.iter().map(|variant| (false, variant.id)))
        .chain(player.emotes.iter().map(|emote| (true, emote.id)))
        .collect()
}

fn watch_purchases(account: Uuid) {
    let started = WATCHING.lock().is_ok_and(|mut watching| {
        let pending = watching.get(&account).map_or(0, |watch| watch.pending);
        watching
            .insert(
                account,
                PurchaseWatch {
                    begun: std::time::Instant::now(),
                    pending: pending + 1,
                },
            )
            .is_none()
    });
    if !started {
        return;
    }
    freya::prelude::spawn_in_window(async move {
        let mut minecraft = None;
        let mut baseline = None;
        loop {
            let expired = WATCHING.lock().map_or(true, |mut watching| {
                let expired = watching
                    .get(&account)
                    .is_none_or(|watch| watch.pending == 0 || watch.begun.elapsed() >= WATCH_LIMIT);
                if expired {
                    watching.remove(&account);
                }
                expired
            });
            if expired {
                break;
            }
            if minecraft.is_none() {
                minecraft = fresh_account(account).await.ok();
            }
            let latest = match &minecraft {
                Some(minecraft) => api::fetch_player(minecraft)
                    .await
                    .ok()
                    .map(|player| owned_ids(&player)),
                None => None,
            };
            match (latest, &baseline) {
                (Some(latest), Some(previous)) if latest != *previous => {
                    QueriesStorage::<PlusCosmeticsQuery>::invalidate_all().await;
                    QueriesStorage::<TransactionsQuery>::invalidate_all().await;
                    if let Ok(mut watching) = WATCHING.lock()
                        && let Some(watch) = watching.get_mut(&account)
                    {
                        watch.pending = watch.pending.saturating_sub(1);
                    }
                    baseline = Some(latest);
                }
                (Some(latest), None) => baseline = Some(latest),
                _ => {}
            }
            tokio::time::sleep(WATCH_INTERVAL).await;
        }
    });
}

pub async fn invalidate_wardrobe_queries() {
    if let Ok(mut profiles) = PROFILES.lock() {
        profiles.clear();
    }
    QueriesStorage::<MojangLookQuery>::invalidate_all().await;
    QueriesStorage::<PlusCosmeticsQuery>::invalidate_all().await;
    QueriesStorage::<OutfitsQuery>::invalidate_all().await;
    QueriesStorage::<SkinsQuery>::invalidate_all().await;
    QueriesStorage::<StoreQuery>::invalidate_all().await;
    QueriesStorage::<TransactionsQuery>::invalidate_all().await;
    QueriesStorage::<FetchPlayerProfileQuery>::invalidate_all().await;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WardrobeAction {
    Equip {
        account: Uuid,
        slot: BodySlot,
        cosmetic: Option<i32>,
        name: String,
    },
    MojangCape {
        account: Uuid,
        cape: Option<String>,
        name: String,
    },
    WearSkin {
        account: Uuid,
        url: String,
        slim: bool,
        name: String,
    },
    AddSkinFile {
        account: Uuid,
        path: PathBuf,
        slim: bool,
        name: Option<String>,
    },
    AddSkinUrl {
        account: Uuid,
        url: String,
        slim: bool,
        name: Option<String>,
    },
    AddSkinUsername {
        account: Uuid,
        username: String,
    },
    Checkout {
        account: Uuid,
        products: Vec<String>,
    },
    BuyNow {
        account: Uuid,
        cosmetic: i32,
        name: String,
    },
    RenameSkin {
        account: Uuid,
        id: i32,
        old: String,
        name: String,
    },
    FavoriteSkin {
        account: Uuid,
        id: i32,
        name: String,
        favorite: bool,
    },
    SkinArms {
        account: Uuid,
        id: i32,
        url: String,
        name: String,
        slim: bool,
    },
    DeleteSkin {
        account: Uuid,
        id: i32,
        name: String,
    },
    ApplyOutfit {
        account: Uuid,
        id: i32,
        name: String,
    },
    CreateOutfit {
        account: Uuid,
    },
    RenameOutfit {
        account: Uuid,
        id: i32,
        old: String,
        name: String,
    },
    FavoriteOutfit {
        account: Uuid,
        id: i32,
        name: String,
        favorite: bool,
    },
    DeleteOutfit {
        account: Uuid,
        id: i32,
        name: String,
    },
    ParticleColor {
        account: Uuid,
        color: Option<i32>,
    },
}

impl WardrobeAction {
    pub fn pending(&self) -> Option<String> {
        match self {
            Self::Equip {
                cosmetic: Some(_),
                name,
                ..
            } => Some(format!("Equipping {name}...")),
            Self::Equip {
                cosmetic: None,
                name,
                ..
            } => Some(format!("Unequipping {name}...")),
            Self::MojangCape {
                cape: Some(_),
                name,
                ..
            } => Some(format!("Equipping {name}...")),
            Self::MojangCape {
                cape: None, name, ..
            } => Some(format!("Removing {name}...")),
            Self::WearSkin { name, .. } => Some(format!("Changing your skin to {name}...")),
            Self::AddSkinFile { .. } | Self::AddSkinUrl { .. } | Self::AddSkinUsername { .. } => {
                Some("Adding skin...".into())
            }
            Self::Checkout { .. } | Self::BuyNow { .. } => Some("Opening checkout...".into()),
            Self::DeleteSkin { name, .. } | Self::DeleteOutfit { name, .. } => {
                Some(format!("Deleting {name}..."))
            }
            Self::ApplyOutfit { name, .. } => Some(format!("Switching to {name}...")),
            Self::CreateOutfit { .. } => Some("Saving your current look...".into()),
            Self::ParticleColor { .. } => Some("Changing your particle color...".into()),
            Self::RenameSkin { .. }
            | Self::FavoriteSkin { .. }
            | Self::SkinArms { .. }
            | Self::RenameOutfit { .. }
            | Self::FavoriteOutfit { .. } => None,
        }
    }

    fn account(&self) -> Uuid {
        match self {
            Self::Equip { account, .. }
            | Self::MojangCape { account, .. }
            | Self::WearSkin { account, .. }
            | Self::AddSkinFile { account, .. }
            | Self::AddSkinUrl { account, .. }
            | Self::AddSkinUsername { account, .. }
            | Self::Checkout { account, .. }
            | Self::BuyNow { account, .. }
            | Self::RenameSkin { account, .. }
            | Self::FavoriteSkin { account, .. }
            | Self::SkinArms { account, .. }
            | Self::DeleteSkin { account, .. }
            | Self::ApplyOutfit { account, .. }
            | Self::CreateOutfit { account }
            | Self::RenameOutfit { account, .. }
            | Self::FavoriteOutfit { account, .. }
            | Self::DeleteOutfit { account, .. }
            | Self::ParticleColor { account, .. } => *account,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WardrobeMutation;

impl MutationCapability for WardrobeMutation {
    type Ok = String;
    type Err = String;
    type Keys = WardrobeAction;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let account = fresh_account(keys.account()).await?;
        run_action(&account, keys).await
    }

    async fn on_settled(&self, keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
        invalidate_after(keys).await;
        if result.is_ok()
            && matches!(
                keys,
                WardrobeAction::Checkout { .. } | WardrobeAction::BuyNow { .. }
            )
        {
            watch_purchases(keys.account());
        }
    }
}

pub type UseWardrobeAction = UseMutation<WardrobeMutation>;

pub fn use_wardrobe_action() -> UseWardrobeAction {
    use_mutation(Mutation::new(WardrobeMutation))
}

enum LookChange {
    Skin(i32),
    Cape(Option<String>),
}

async fn sync_active_outfit(
    account: &MinecraftAccount,
    profile: &MojangFullPlayerProfile,
    change: LookChange,
) -> bool {
    let list = match api::list_outfits(account).await {
        Ok(list) => list,
        Err(PlusError::Api { status: 404, .. }) => return true,
        Err(err) => {
            tracing::warn!("[wardrobe] could not load the outfits to update the active one: {err}");
            return false;
        }
    };
    let Some(id) = list.active_outfit_id else {
        return true;
    };
    if list
        .outfits
        .iter()
        .any(|outfit| outfit.id == id && outfit.blank)
    {
        return adopt_current_look(account, profile, id, false).await != Adoption::Failed;
    }
    let update = match change {
        LookChange::Skin(skin) => api::OutfitUpdate {
            skin_id: Some(Some(skin)),
            ..Default::default()
        },
        LookChange::Cape(cape) => api::OutfitUpdate {
            mojang_cape_id: Some(cape),
            ..Default::default()
        },
    };
    match api::update_outfit(account, id, &update).await {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!("[wardrobe] could not update the active outfit: {err}");
            false
        }
    }
}

fn synced(synced: bool, done: String) -> String {
    if synced {
        format!("{done}.")
    } else {
        format!("{done}, but your active outfit couldn't be updated.")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Adoption {
    Saved,
    SkinUnknown(String),
    Failed,
}

async fn adopt_current_look(
    account: &MinecraftAccount,
    profile: &MojangFullPlayerProfile,
    outfit: i32,
    force: bool,
) -> Adoption {
    let skin = match record_current_skin(account, profile, force).await {
        Ok(skin) => skin,
        Err(reason) => {
            tracing::warn!(
                "[wardrobe] could not record the current skin, left outfit {outfit} blank: {reason}"
            );
            return Adoption::SkinUnknown(reason);
        }
    };
    let update = api::OutfitUpdate {
        skin_id: Some(skin),
        mojang_cape_id: Some(active_cape(profile).map(|cape| cape.id.clone())),
        blank: Some(false),
        ..Default::default()
    };
    match api::update_outfit(account, outfit, &update).await {
        Ok(_) => Adoption::Saved,
        Err(err) => {
            tracing::warn!(
                "[wardrobe] could not save the current look into outfit {outfit}: {err}"
            );
            Adoption::Failed
        }
    }
}

async fn remember_profile(account: &MinecraftAccount, profile: MojangFullPlayerProfile) {
    cache_profile(account.id, &profile);
    oneclient_mc::store_profile_view(&account.id.to_string(), profile).await;
}

async fn save_worn_skin(
    account: &MinecraftAccount,
    profile: MojangFullPlayerProfile,
    name: Option<&str>,
) -> Result<String, String> {
    let worn = active_skin(&profile).and_then(|skin| {
        api::texture_hash(&skin.url).map(|hash| (hash, skin_model(&skin.variant)))
    });
    remember_profile(account, profile.clone()).await;
    let Some((hash, model)) = worn else {
        return Ok("Added the skin and put it on.".into());
    };
    let mut recorded = RECORDED.lock().await;
    let info = match add_worn_skin(account, &hash, model, name).await {
        Ok(info) => info,
        Err(err) => {
            recorded.failed((account.id, hash), &err);
            return Err(format!(
                "Put the skin on, but couldn't save it to your library. {}",
                plus_message(err)
            ));
        }
    };
    recorded.saved((account.id, hash), info.id);
    drop(recorded);
    Ok(synced(
        sync_active_outfit(account, &profile, LookChange::Skin(info.id)).await,
        "Added the skin and put it on".into(),
    ))
}

async fn run_action(account: &MinecraftAccount, action: &WardrobeAction) -> Result<String, String> {
    let state = crate::launcher::state().map_err(|err| err.to_string())?;
    let requester = &state.services.requester;

    match action {
        WardrobeAction::Equip {
            slot,
            cosmetic,
            name,
            ..
        } => {
            let changes = HashMap::from([(*slot, *cosmetic)]);
            api::set_equipped(account, &changes)
                .await
                .map_err(plus_message)?;
            Ok(if cosmetic.is_some() {
                format!("Equipped {name}.")
            } else {
                format!("Unequipped {name}.")
            })
        }
        WardrobeAction::MojangCape { cape, name, .. } => {
            let profile = match cape {
                Some(id) => oneclient_mc::select_cape(requester, &account.access_token, id).await,
                None => oneclient_mc::hide_cape(requester, &account.access_token).await,
            }
            .map_err(mojang_write_failed(account.id))?;
            remember_profile(account, profile.clone()).await;
            Ok(synced(
                sync_active_outfit(account, &profile, LookChange::Cape(cape.clone())).await,
                if cape.is_some() {
                    format!("Equipped {name}")
                } else {
                    format!("Removed {name}")
                },
            ))
        }
        WardrobeAction::WearSkin {
            url, slim, name, ..
        } => {
            let variant = if *slim {
                SkinVariant::Slim
            } else {
                SkinVariant::Classic
            };
            let profile =
                oneclient_mc::set_skin_from_url(requester, &account.access_token, url, variant)
                    .await
                    .map_err(mojang_write_failed(account.id))?;
            remember_profile(account, profile.clone()).await;
            Ok(match record_current_skin(account, &profile, true).await {
                Ok(Some(skin)) => synced(
                    sync_active_outfit(account, &profile, LookChange::Skin(skin)).await,
                    format!("Wearing {name}"),
                ),
                Ok(None) => format!("Wearing {name}."),
                Err(reason) => {
                    format!("Wearing {name}, but it couldn't be saved to your library. {reason}")
                }
            })
        }
        WardrobeAction::AddSkinFile {
            path, slim, name, ..
        } => {
            let png = tokio::fs::read(path)
                .await
                .map_err(|err| format!("Couldn't read that file ({err})."))?;
            if !png.starts_with(b"\x89PNG") {
                return Err("Skins must be PNG images.".into());
            }
            let variant = if *slim {
                SkinVariant::Slim
            } else {
                SkinVariant::Classic
            };
            let profile = oneclient_mc::upload_skin(requester, &account.access_token, png, variant)
                .await
                .map_err(mojang_write_failed(account.id))?;
            let name = name.clone().or_else(|| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().chars().take(64).collect::<String>())
                    .filter(|stem| !stem.trim().is_empty())
            });
            save_worn_skin(account, profile, name.as_deref()).await
        }
        WardrobeAction::AddSkinUrl {
            url, slim, name, ..
        } => {
            let variant = if *slim {
                SkinVariant::Slim
            } else {
                SkinVariant::Classic
            };
            let profile =
                oneclient_mc::set_skin_from_url(requester, &account.access_token, url, variant)
                    .await
                    .map_err(mojang_write_failed(account.id))?;
            save_worn_skin(account, profile, name.as_deref()).await
        }
        WardrobeAction::AddSkinUsername { username, .. } => {
            let (uuid, exact) = oneclient_mc::resolve_username(requester, username)
                .await
                .map_err(|err| {
                    if matches!(err, oneclient_mc::McError::Minecraft(_))
                        || matches!(err.http_status(), Some(204 | 404))
                    {
                        format!("Couldn't find a Minecraft player called {username}.")
                    } else {
                        mojang_read_message(err)
                    }
                })?;
            let profile = oneclient_mc::fetch_player_profile(requester, &uuid)
                .await
                .map_err(mojang_read_message)?;
            let Some(hash) = profile.skin_url.as_deref().and_then(api::texture_hash) else {
                return Err(format!("{exact} uses the default skin."));
            };
            let model = if profile.is_slim {
                SkinModel::Slim
            } else {
                SkinModel::Classic
            };
            api::add_skin(account, &hash, model, Some(&exact))
                .await
                .map_err(plus_message)?;
            Ok(format!("Saved {exact}'s skin."))
        }
        WardrobeAction::RenameSkin { id, old, name, .. } => {
            let skin = api::update_skin(account, *id, &serde_json::json!({ "name": name.trim() }))
                .await
                .map_err(plus_message)?;
            Ok(format!("Renamed {old} to {}.", skin.name))
        }
        WardrobeAction::FavoriteSkin {
            id, name, favorite, ..
        } => {
            api::update_skin(account, *id, &serde_json::json!({ "favorite": favorite }))
                .await
                .map_err(plus_message)?;
            Ok(if *favorite {
                format!("Added {name} to favourites.")
            } else {
                format!("Removed {name} from favourites.")
            })
        }
        WardrobeAction::SkinArms {
            id,
            url,
            name,
            slim,
            ..
        } => {
            let model = if *slim { "slim" } else { "classic" };
            let worn = worn_hash(&fresh_profile(account).await?)
                .is_some_and(|hash| api::texture_hash(url).as_deref() == Some(hash.as_str()));
            if worn {
                let variant = if *slim {
                    SkinVariant::Slim
                } else {
                    SkinVariant::Classic
                };
                let profile =
                    oneclient_mc::set_skin_from_url(requester, &account.access_token, url, variant)
                        .await
                        .map_err(mojang_write_failed(account.id))?;
                remember_profile(account, profile).await;
            }
            api::update_skin(account, *id, &serde_json::json!({ "model": model }))
                .await
                .map_err(plus_message)?;
            Ok(format!("{name} now uses {model} arms."))
        }
        WardrobeAction::DeleteSkin { id, name, .. } => {
            api::delete_skin(account, *id).await.map_err(plus_message)?;
            forget_recorded(account.id, *id).await;
            Ok(format!("Deleted {name}."))
        }
        WardrobeAction::ApplyOutfit { id, name, .. } => apply_outfit(account, *id, name).await,
        WardrobeAction::Checkout { products, .. } => checkout(account, products).await,
        WardrobeAction::BuyNow { cosmetic, name, .. } => {
            let product = api::store_product_id(*cosmetic)
                .await
                .map_err(plus_message)?
                .ok_or_else(|| format!("{name} isn't purchasable yet."))?;
            checkout(account, &[product]).await
        }
        WardrobeAction::CreateOutfit { .. } => {
            let profile = mojang_profile(account).await?;
            let skin_id = record_current_skin(account, &profile, true)
                .await
                .map_err(|reason| {
                    format!(
                        "Couldn't save your current skin to your library, so the outfit wasn't saved. {reason}"
                    )
                })?;
            let cape_id = active_cape(&profile).map(|cape| cape.id.clone());
            let particle_color = api::fetch_player(account)
                .await
                .ok()
                .and_then(|player| player.particle_color);
            let outfit = api::create_outfit(account, skin_id, cape_id.as_deref(), particle_color)
                .await
                .map_err(plus_message)?;
            Ok(format!("Saved {}.", outfit.name))
        }
        WardrobeAction::RenameOutfit { id, old, name, .. } => {
            let update = api::OutfitUpdate {
                name: Some(name.trim().to_string()),
                ..Default::default()
            };
            let outfit = api::update_outfit(account, *id, &update)
                .await
                .map_err(plus_message)?;
            Ok(format!("Renamed {old} to {}.", outfit.name))
        }
        WardrobeAction::FavoriteOutfit {
            id, name, favorite, ..
        } => {
            let update = api::OutfitUpdate {
                favorite: Some(*favorite),
                ..Default::default()
            };
            api::update_outfit(account, *id, &update)
                .await
                .map_err(plus_message)?;
            Ok(if *favorite {
                format!("Added {name} to favourites.")
            } else {
                format!("Removed {name} from favourites.")
            })
        }
        WardrobeAction::DeleteOutfit { id, name, .. } => {
            api::delete_outfit(account, *id)
                .await
                .map_err(plus_message)?;
            Ok(format!("Deleted {name}."))
        }
        WardrobeAction::ParticleColor { color, .. } => {
            let id = api::list_outfits(account)
                .await
                .map_err(plus_message)?
                .active_outfit_id
                .ok_or_else(|| "Switch to an outfit to change your particle color.".to_string())?;
            let update = api::OutfitUpdate {
                particle_color: Some(*color),
                ..Default::default()
            };
            api::update_outfit(account, id, &update)
                .await
                .map_err(plus_message)?;
            Ok(if color.is_some() {
                "Changed your particle color.".into()
            } else {
                "Reset your particle color.".into()
            })
        }
    }
}

async fn checkout(account: &MinecraftAccount, products: &[String]) -> Result<String, String> {
    if products.is_empty() {
        return Err("Nothing purchasable in your cart yet.".into());
    }
    let url = api::create_checkout(account, products)
        .await
        .map_err(|err| match err {
            PlusError::Api { status: 409, body } if !body.trim().is_empty() => {
                body.trim().to_string()
            }
            PlusError::Api { status: 409, .. } => "You already own this item.".into(),
            other => plus_message(other),
        })?;
    crate::platform::open_url(&url);
    Ok("Checkout opened in your browser.".into())
}

async fn apply_outfit(account: &MinecraftAccount, id: i32, name: &str) -> Result<String, String> {
    let state = crate::launcher::state().map_err(|err| err.to_string())?;
    let requester = &state.services.requester;

    let mut profile = fresh_profile(account).await?;
    let applied = api::apply_outfit(account, id).await.map_err(plus_message)?;

    if applied.adopted {
        return match adopt_current_look(account, &profile, id, true).await {
            Adoption::Saved => Ok(format!("Wearing {name}.")),
            Adoption::SkinUnknown(reason) => Err(format!(
                "Switched to {name}, but your current skin couldn't be saved into it. {reason}"
            )),
            Adoption::Failed => Err(format!(
                "Switched to {name}, but your current skin and cape couldn't be saved into it."
            )),
        };
    }
    if applied.outfit.blank {
        return Ok(format!("Wearing {name}."));
    }

    let mut problems = Vec::new();
    if let Some(skin) = &applied.skin {
        let wanted_slim = skin.model == SkinModel::Slim;
        let current = active_skin(&profile);
        let same = current.is_some_and(|current| {
            api::texture_hash(&current.url).as_deref() == Some(skin.hash.as_str())
                && (current.variant == SkinVariant::Slim) == wanted_slim
        });
        if !same {
            let variant = if wanted_slim {
                SkinVariant::Slim
            } else {
                SkinVariant::Classic
            };
            match oneclient_mc::set_skin_from_url(
                requester,
                &account.access_token,
                &skin.url,
                variant,
            )
            .await
            {
                Ok(updated) => profile = updated,
                Err(err) => problems.push(format!(
                    "your Minecraft skin wasn't changed ({})",
                    mojang_message(err)
                )),
            }
        }
    }

    let wanted_cape = applied.outfit.mojang_cape_id.clone();
    let current_cape = active_cape(&profile).map(|cape| cape.id.clone());
    if wanted_cape != current_cape {
        let result = match &wanted_cape {
            Some(cape) => oneclient_mc::select_cape(requester, &account.access_token, cape).await,
            None => oneclient_mc::hide_cape(requester, &account.access_token).await,
        };
        match result {
            Ok(updated) => profile = updated,
            Err(err) => problems.push(format!(
                "your Minecraft cape wasn't changed ({})",
                mojang_message(err)
            )),
        }
    }

    remember_profile(account, profile).await;

    if problems.is_empty() {
        Ok(format!("Wearing {name}."))
    } else {
        forget_profile(account.id);
        Err(format!("Outfit applied, but {}.", problems.join(" and ")))
    }
}
