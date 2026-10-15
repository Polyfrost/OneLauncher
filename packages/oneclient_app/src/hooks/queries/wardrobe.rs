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

type RecordedSkins = HashMap<(Uuid, String), Option<i32>>;

static RECORDED: std::sync::LazyLock<tokio::sync::Mutex<RecordedSkins>> =
    std::sync::LazyLock::new(|| tokio::sync::Mutex::new(HashMap::new()));

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

fn plus_message(err: PlusError) -> String {
    match err {
        PlusError::ConsentRequired => {
            "PolyPlus needs the Terms of Service and Privacy Policy. Accept them in OneClient's settings."
                .into()
        }
        PlusError::OfflineAccount => "Sign in with a Microsoft account to use PolyPlus.".into(),
        PlusError::Api { status: 404, .. } => "That item no longer exists.".into(),
        PlusError::Api { status: 409, .. } => {
            "You've reached the limit. Delete one to make room.".into()
        }
        PlusError::Api { status: 429, .. } => {
            "Too many changes at once. Try again in a moment.".into()
        }
        PlusError::Api { status, .. } if status >= 500 => {
            "The PolyPlus servers are having trouble. Try again later.".into()
        }
        PlusError::Api { body, .. } if !body.trim().is_empty() => body.trim().to_string(),
        PlusError::Request(_) | PlusError::Websocket(_) => {
            "Couldn't reach the PolyPlus servers.".into()
        }
        other => other.to_string(),
    }
}

fn list_message(err: PlusError, what: &str) -> String {
    match err {
        PlusError::Api { status: 404, .. } => {
            format!("{what} aren't available on the PolyPlus servers yet.")
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
    Ok(account)
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
    .map_err(mojang_message)?;
    cache_profile(account.id, &profile);
    Ok(profile)
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
) -> Option<i32> {
    let skin = active_skin(profile)?;
    let hash = api::texture_hash(&skin.url)?;
    let key = (account.id, hash);
    let mut recorded = RECORDED.lock().await;
    if !force && let Some(id) = recorded.get(&key) {
        return *id;
    }
    match api::add_skin(account, &key.1, skin_model(&skin.variant), None).await {
        Ok(info) => {
            recorded.insert(key, Some(info.id));
            Some(info.id)
        }
        Err(err) => {
            tracing::warn!("[wardrobe] could not record the current skin: {err}");
            recorded.entry(key).or_insert(None);
            None
        }
    }
}

async fn already_recorded(account: Uuid, profile: &MojangFullPlayerProfile) -> bool {
    let Some(hash) = active_skin(profile).and_then(|skin| api::texture_hash(&skin.url)) else {
        return true;
    };
    RECORDED.lock().await.contains_key(&(account, hash))
}

async fn forget_recorded(account: Uuid, skin: i32) {
    for (key, id) in RECORDED.lock().await.iter_mut() {
        if key.0 == account && *id == Some(skin) {
            *id = None;
        }
    }
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

        let active_blank = list
            .active_outfit_id
            .and_then(|id| list.outfits.iter().find(|outfit| outfit.id == id))
            .filter(|outfit| outfit.blank)
            .map(|outfit| outfit.id);
        if let Some(id) = active_blank
            && let Ok(profile) = mojang_profile(&account).await
            && adopt_current_look(&account, &profile, id, false).await
        {
            return api::list_outfits(&account)
                .await
                .map_err(|err| list_message(err, "Outfits"));
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
            && record_current_skin(&account, &profile, false)
                .await
                .is_some()
        {
            return api::list_skins(&account)
                .await
                .map_err(|err| list_message(err, "Skins"));
        }
        Ok(list)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CapeTextureKeys {
    pub url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CapeTextureQuery;

impl QueryCapability for CapeTextureQuery {
    type Ok = Bytes;
    type Err = String;
    type Keys = CapeTextureKeys;

    async fn run(&self, keys: &Self::Keys) -> Result<Self::Ok, Self::Err> {
        let Some(url) = keys.url.as_deref() else {
            return Err("No cape".into());
        };
        api::cape_texture(url).await.map_err(plus_message)
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
            .find(|item| item.variant_list().iter().any(|entry| entry.id == variant))
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

pub fn use_cape_texture(url: Option<String>) -> UseQuery<CapeTextureQuery> {
    use_query(Query::new(CapeTextureKeys { url }, CapeTextureQuery).stale_time(TEXTURE_STALE))
}

async fn invalidate_after(action: &WardrobeAction) {
    let (look, plus, outfits, skins, transactions) = match action {
        WardrobeAction::Equip { .. } => (false, true, true, false, false),
        WardrobeAction::MojangCape { .. } => (true, false, true, false, false),
        WardrobeAction::WearSkin { .. }
        | WardrobeAction::AddSkinFile { .. }
        | WardrobeAction::AddSkinUrl { .. } => (true, false, true, true, false),
        WardrobeAction::AddSkinUsername { .. }
        | WardrobeAction::RenameSkin { .. }
        | WardrobeAction::FavoriteSkin { .. }
        | WardrobeAction::SkinArms { .. } => (false, false, false, true, false),
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

static WATCHING: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<Uuid>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

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
    let started = WATCHING
        .lock()
        .is_ok_and(|mut watching| watching.insert(account));
    if !started {
        return;
    }
    freya::prelude::spawn_forever(async move {
        if let Ok(minecraft) = fresh_account(account).await
            && let Ok(player) = api::fetch_player(&minecraft).await
        {
            let baseline = owned_ids(&player);
            let begun = std::time::Instant::now();
            while begun.elapsed() < WATCH_LIMIT {
                tokio::time::sleep(WATCH_INTERVAL).await;
                let Ok(latest) = api::fetch_player(&minecraft).await else {
                    continue;
                };
                if owned_ids(&latest) != baseline {
                    QueriesStorage::<PlusCosmeticsQuery>::invalidate_all().await;
                    QueriesStorage::<TransactionsQuery>::invalidate_all().await;
                    break;
                }
            }
        }
        if let Ok(mut watching) = WATCHING.lock() {
            watching.remove(&account);
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
            | Self::DeleteOutfit { account, .. } => *account,
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

async fn sync_active_outfit(account: &MinecraftAccount, changes: serde_json::Value) {
    let Ok(list) = api::list_outfits(account).await else {
        return;
    };
    let Some(id) = list.active_outfit_id else {
        return;
    };
    if let Err(err) = api::update_outfit(account, id, &changes).await {
        tracing::warn!("[wardrobe] could not update the active outfit: {err}");
    }
}

async fn adopt_current_look(
    account: &MinecraftAccount,
    profile: &MojangFullPlayerProfile,
    outfit: i32,
    force: bool,
) -> bool {
    let mut changes = serde_json::Map::new();
    let has_skin = active_skin(profile)
        .and_then(|skin| api::texture_hash(&skin.url))
        .is_some();
    match record_current_skin(account, profile, force).await {
        Some(skin) => {
            changes.insert("skin_id".into(), skin.into());
        }
        None if has_skin => {
            tracing::warn!(
                "[wardrobe] could not record the current skin, leaving outfit {outfit} untouched"
            );
            return false;
        }
        None => {}
    }
    changes.insert(
        "mojang_cape_id".into(),
        active_cape(profile).map(|cape| cape.id.clone()).into(),
    );
    match api::update_outfit(account, outfit, &serde_json::Value::Object(changes)).await {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!(
                "[wardrobe] could not save the current look into outfit {outfit}: {err}"
            );
            false
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
    remember_profile(account, profile).await;
    let Some((hash, model)) = worn else {
        return Ok("Added the skin and put it on.".into());
    };
    let mut recorded = RECORDED.lock().await;
    let info = match api::add_skin(account, &hash, model, name).await {
        Ok(info) => info,
        Err(err) => {
            recorded.entry((account.id, hash)).or_insert(None);
            return Err(format!(
                "Put the skin on, but couldn't save it to your library. {}",
                plus_message(err)
            ));
        }
    };
    recorded.insert((account.id, hash), Some(info.id));
    drop(recorded);
    sync_active_outfit(account, serde_json::json!({ "skin_id": info.id })).await;
    Ok("Added the skin and put it on.".into())
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
            .map_err(mojang_message)?;
            remember_profile(account, profile).await;
            sync_active_outfit(account, serde_json::json!({ "mojang_cape_id": cape })).await;
            Ok(if cape.is_some() {
                format!("Equipped {name}.")
            } else {
                format!("Removed {name}.")
            })
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
                    .map_err(mojang_message)?;
            if let Some(skin) = record_current_skin(account, &profile, true).await {
                sync_active_outfit(account, serde_json::json!({ "skin_id": skin })).await;
            }
            remember_profile(account, profile).await;
            Ok(format!("Wearing {name}."))
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
                .map_err(mojang_message)?;
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
                    .map_err(mojang_message)?;
            save_worn_skin(account, profile, name.as_deref()).await
        }
        WardrobeAction::AddSkinUsername { username, .. } => {
            let (uuid, exact) = oneclient_mc::resolve_username(requester, username)
                .await
                .map_err(|_| format!("Couldn't find a Minecraft player called {username}."))?;
            let profile = oneclient_mc::fetch_player_profile(requester, &uuid)
                .await
                .map_err(mojang_message)?;
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
        WardrobeAction::SkinArms { id, name, slim, .. } => {
            let model = if *slim { "slim" } else { "classic" };
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
            let profile = mojang_profile(account).await.ok();
            let skin_id = match &profile {
                Some(profile) => record_current_skin(account, profile, true).await,
                None => None,
            };
            let cape_id = profile
                .as_ref()
                .and_then(active_cape)
                .map(|cape| cape.id.clone());
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
            let outfit =
                api::update_outfit(account, *id, &serde_json::json!({ "name": name.trim() }))
                    .await
                    .map_err(plus_message)?;
            Ok(format!("Renamed {old} to {}.", outfit.name))
        }
        WardrobeAction::FavoriteOutfit {
            id, name, favorite, ..
        } => {
            api::update_outfit(account, *id, &serde_json::json!({ "favorite": favorite }))
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

    let applied = api::apply_outfit(account, id).await.map_err(plus_message)?;
    let mut profile = mojang_profile(account).await?;

    if applied.outfit.blank {
        adopt_current_look(account, &profile, id, true).await;
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
        Err(format!("Outfit applied, but {}.", problems.join(" and ")))
    }
}
