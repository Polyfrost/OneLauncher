use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use bytes::Bytes;
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use oneclient_auth::MinecraftAccount;

use crate::{PlusError, base_url, build_client, login, plus_blocked};

const TOKEN_LIFETIME: Duration = Duration::from_secs(100 * 60);

static CLIENT: LazyLock<Result<reqwest::Client, String>> =
    LazyLock::new(|| build_client().map_err(|err| err.to_string()));

static TOKENS: LazyLock<Mutex<HashMap<Uuid, (String, Instant)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum BodySlot {
    Cape,
    Backpack,
    Glasses,
    Wings,
    LeftHand,
    RightHand,
    Hat,
    Aura,
    Boots,
    Shoulder,
    Pet,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CosmeticType {
    Cape,
    Emote,
    Backpack,
    Glasses,
    Wings,
    Glove,
    Hat,
    Aura,
    Boots,
    Shoulder,
    Pet,
    #[serde(other)]
    Unknown,
}

impl CosmeticType {
    pub fn default_slot(self) -> Option<BodySlot> {
        match self {
            Self::Cape => Some(BodySlot::Cape),
            Self::Backpack => Some(BodySlot::Backpack),
            Self::Glasses => Some(BodySlot::Glasses),
            Self::Wings => Some(BodySlot::Wings),
            Self::Glove => Some(BodySlot::RightHand),
            Self::Hat => Some(BodySlot::Hat),
            Self::Aura => Some(BodySlot::Aura),
            Self::Boots => Some(BodySlot::Boots),
            Self::Shoulder => Some(BodySlot::Shoulder),
            Self::Pet => Some(BodySlot::Pet),
            Self::Emote | Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CosmeticVariant {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub allowed_slots: Vec<BodySlot>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CosmeticGroup {
    pub id: i32,
    #[serde(rename = "type")]
    pub kind: CosmeticType,
    pub name: String,
    #[serde(default)]
    pub allowed_slots: Vec<BodySlot>,
    #[serde(default)]
    pub variants: Vec<CosmeticVariant>,
}

impl CosmeticGroup {
    pub fn ui_variants(&self) -> Vec<&CosmeticVariant> {
        let mut seen: Vec<&str> = Vec::new();
        let mut out = Vec::new();
        for variant in &self.variants {
            if !seen.contains(&variant.name.as_str()) {
                seen.push(variant.name.as_str());
                out.push(variant);
            }
        }
        out
    }

    pub fn variant_for_skin(&self, variant_id: i32, slim: bool) -> i32 {
        let Some(current) = self.variants.iter().find(|v| v.id == variant_id) else {
            return variant_id;
        };
        if current.model.is_none() {
            return variant_id;
        }
        let wanted = if slim { "slim" } else { "wide" };
        self.variants
            .iter()
            .find(|v| v.name == current.name && v.model.as_deref() == Some(wanted))
            .map_or(variant_id, |v| v.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EmoteInfo {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct PlayerCosmetics {
    #[serde(default)]
    pub cosmetics: Vec<CosmeticGroup>,
    #[serde(default)]
    pub emotes: Vec<EmoteInfo>,
    #[serde(default)]
    pub equipped: HashMap<BodySlot, i32>,
    #[serde(default)]
    pub particle_color: Option<i32>,
    #[serde(default)]
    pub expiring: HashMap<i32, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkinModel {
    Classic,
    Slim,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SkinInfo {
    pub id: i32,
    pub name: String,
    pub model: SkinModel,
    pub url: String,
    pub hash: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SkinList {
    pub skins: Vec<SkinInfo>,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OutfitInfo {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub skin_id: Option<i32>,
    #[serde(default)]
    pub mojang_cape_id: Option<String>,
    #[serde(default)]
    pub particle_color: Option<i32>,
    #[serde(default)]
    pub blank: bool,
    #[serde(default)]
    pub equipped: HashMap<BodySlot, i32>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OutfitList {
    pub outfits: Vec<OutfitInfo>,
    pub active_outfit_id: Option<i32>,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppliedOutfit {
    pub outfit: OutfitInfo,
    pub skin: Option<SkinInfo>,
}

fn client() -> Result<&'static reqwest::Client, PlusError> {
    CLIENT
        .as_ref()
        .map_err(|err| PlusError::Client(err.clone()))
}

async fn token_for(account: &MinecraftAccount, fresh: bool) -> Result<String, PlusError> {
    let mut tokens = TOKENS.lock().await;
    if !fresh
        && let Some((token, issued)) = tokens.get(&account.id)
        && issued.elapsed() < TOKEN_LIFETIME
    {
        return Ok(token.clone());
    }

    let token = login(client()?, account).await?;
    tokens.insert(account.id, (token.clone(), Instant::now()));
    Ok(token)
}

async fn send_authorized(
    account: &MinecraftAccount,
    method: Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<reqwest::Response, PlusError> {
    if plus_blocked() {
        return Err(PlusError::ConsentRequired);
    }
    if !account.is_microsoft() {
        return Err(PlusError::OfflineAccount);
    }

    let url = format!("{}{path}", base_url());
    let mut fresh = false;
    loop {
        let token = token_for(account, fresh).await?;
        let mut request = client()?
            .request(method.clone(), &url)
            .bearer_auth(&token)
            .header("Accept", "application/json");
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await?;
        if response.status() == StatusCode::UNAUTHORIZED && !fresh {
            fresh = true;
            continue;
        }
        if !response.status().is_success() {
            return Err(PlusError::Api {
                status: response.status().as_u16(),
                body: response.text().await.unwrap_or_default(),
            });
        }
        return Ok(response);
    }
}

async fn json_authorized<T: DeserializeOwned>(
    account: &MinecraftAccount,
    method: Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<T, PlusError> {
    Ok(send_authorized(account, method, path, body)
        .await?
        .json::<T>()
        .await?)
}

pub async fn fetch_player(account: &MinecraftAccount) -> Result<PlayerCosmetics, PlusError> {
    json_authorized(account, Method::GET, "/cosmetics/player", None).await
}

pub async fn set_equipped(
    account: &MinecraftAccount,
    changes: &HashMap<BodySlot, Option<i32>>,
) -> Result<(), PlusError> {
    let body = serde_json::json!({ "equipped": changes });
    send_authorized(account, Method::PUT, "/cosmetics/player", Some(&body)).await?;
    Ok(())
}

pub async fn list_outfits(account: &MinecraftAccount) -> Result<OutfitList, PlusError> {
    json_authorized(account, Method::GET, "/outfits", None).await
}

pub async fn create_outfit(
    account: &MinecraftAccount,
    skin_id: Option<i32>,
    mojang_cape_id: Option<&str>,
    particle_color: Option<i32>,
) -> Result<OutfitInfo, PlusError> {
    let body = serde_json::json!({
        "skin_id": skin_id,
        "mojang_cape_id": mojang_cape_id,
        "particle_color": particle_color,
    });
    json_authorized(account, Method::POST, "/outfits", Some(&body)).await
}

pub async fn update_outfit(
    account: &MinecraftAccount,
    id: i32,
    changes: &serde_json::Value,
) -> Result<OutfitInfo, PlusError> {
    json_authorized(
        account,
        Method::PATCH,
        &format!("/outfits/{id}"),
        Some(changes),
    )
    .await
}

pub async fn delete_outfit(account: &MinecraftAccount, id: i32) -> Result<(), PlusError> {
    send_authorized(account, Method::DELETE, &format!("/outfits/{id}"), None).await?;
    Ok(())
}

pub async fn apply_outfit(account: &MinecraftAccount, id: i32) -> Result<AppliedOutfit, PlusError> {
    json_authorized(account, Method::POST, &format!("/outfits/{id}/apply"), None).await
}

pub async fn list_skins(account: &MinecraftAccount) -> Result<SkinList, PlusError> {
    json_authorized(account, Method::GET, "/skins", None).await
}

pub async fn add_skin(
    account: &MinecraftAccount,
    texture_hash: &str,
    model: SkinModel,
    name: Option<&str>,
) -> Result<SkinInfo, PlusError> {
    let body = serde_json::json!({
        "texture_hash": texture_hash,
        "model": model,
        "name": name,
    });
    json_authorized(account, Method::POST, "/skins", Some(&body)).await
}

pub async fn update_skin(
    account: &MinecraftAccount,
    id: i32,
    changes: &serde_json::Value,
) -> Result<SkinInfo, PlusError> {
    json_authorized(
        account,
        Method::PATCH,
        &format!("/skins/{id}"),
        Some(changes),
    )
    .await
}

pub async fn delete_skin(account: &MinecraftAccount, id: i32) -> Result<(), PlusError> {
    send_authorized(account, Method::DELETE, &format!("/skins/{id}"), None).await?;
    Ok(())
}

pub async fn download(url: &str) -> Result<Bytes, PlusError> {
    let response = client()?.get(url).send().await?;
    if !response.status().is_success() {
        return Err(PlusError::Api {
            status: response.status().as_u16(),
            body: String::new(),
        });
    }
    Ok(response.bytes().await?)
}

pub async fn cape_texture(url: &str) -> Result<Bytes, PlusError> {
    let bytes = download(url).await?;
    if !bytes.starts_with(b"PK") {
        return Ok(bytes);
    }

    let entry = polyio::read_zip_bytes_entry(bytes.to_vec(), |name| {
        let file = name.rsplit('/').next().unwrap_or(name);
        !name.contains("__MACOSX")
            && !file.starts_with("._")
            && file.to_ascii_lowercase().ends_with(".png")
    })
    .await
    .map_err(|err| PlusError::Asset(err.to_string()))?;

    entry
        .map(|(_, data)| Bytes::from(data))
        .ok_or_else(|| PlusError::Asset("the cape has no texture".into()))
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub struct StoreTags {
    #[serde(default)]
    pub colors: Vec<String>,
    #[serde(default)]
    pub custom: Vec<String>,
    #[serde(default)]
    pub category: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StoreVariant {
    pub id: i32,
    #[serde(default)]
    pub variant_name: Option<String>,
    #[serde(default)]
    pub model_variant: Option<String>,
    #[serde(default)]
    pub variant_order: i32,
    #[serde(default)]
    pub asset_id: Option<i32>,
    #[serde(default)]
    pub cover_asset_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SaleInfo {
    #[serde(default)]
    pub price: f32,
    #[serde(default)]
    pub discount: f32,
    #[serde(default)]
    pub percent: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ends_at: Option<String>,
}

fn sale_final_price(base: Option<f32>, sale: Option<&SaleInfo>) -> Option<f32> {
    match sale {
        Some(sale) => Some(sale.price),
        None => base,
    }
}

fn sale_percent(sale: Option<&SaleInfo>) -> Option<i32> {
    sale.map(|sale| sale.percent).filter(|percent| *percent > 0)
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StoreItem {
    pub id: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: CosmeticType,
    #[serde(default)]
    pub base_price: Option<f32>,
    #[serde(default)]
    pub sale: Option<SaleInfo>,
    #[serde(default)]
    pub asset_id: Option<i32>,
    #[serde(default)]
    pub cover_asset_id: Option<i32>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub tags: StoreTags,
    #[serde(default)]
    pub variants: Option<Vec<StoreVariant>>,
}

impl StoreItem {
    pub fn variant_list(&self) -> Vec<StoreVariant> {
        match &self.variants {
            Some(variants) if !variants.is_empty() => variants.clone(),
            _ => vec![StoreVariant {
                id: self.id,
                variant_name: Some(self.name.clone()),
                model_variant: None,
                variant_order: 0,
                asset_id: self.asset_id,
                cover_asset_id: self.cover_asset_id,
            }],
        }
    }

    pub fn ui_variants(&self) -> Vec<StoreVariant> {
        let mut list = self.variant_list();
        list.sort_by_key(|variant| variant.variant_order);
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for variant in list {
            let label = variant
                .variant_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or(self.name.as_str())
                .to_string();
            if !seen.contains(&label) {
                seen.push(label);
                out.push(variant);
            }
        }
        out
    }

    pub fn final_price(&self) -> Option<f32> {
        sale_final_price(self.base_price, self.sale.as_ref())
    }

    pub fn discount_percent(&self) -> Option<i32> {
        sale_percent(self.sale.as_ref())
    }

    pub fn discounted(&self) -> bool {
        self.discount_percent().is_some() && self.final_price().unwrap_or(0.) > 0.
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct StoreSearch {
    #[serde(default)]
    results: Vec<StoreItem>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BundleInfo {
    pub id: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub asset_id: Option<i32>,
    #[serde(default)]
    pub base_price: Option<f32>,
    #[serde(default)]
    pub sale: Option<SaleInfo>,
    #[serde(default)]
    pub store_product_id: Option<String>,
}

impl BundleInfo {
    pub fn final_price(&self) -> Option<f32> {
        sale_final_price(self.base_price, self.sale.as_ref())
    }

    pub fn discount_percent(&self) -> Option<i32> {
        sale_percent(self.sale.as_ref())
    }

    pub fn discounted(&self) -> bool {
        self.discount_percent().is_some() && self.final_price().unwrap_or(0.) > 0.
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct BundleSearch {
    #[serde(default)]
    bundles: Vec<BundleInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StoreView {
    #[serde(default)]
    store_product_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CheckoutResponse {
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Transaction {
    pub id: i32,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub amount_minor: Option<i64>,
    #[serde(default)]
    pub currency: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Transactions {
    #[serde(default)]
    transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoreSort {
    Popularity,
    Newest,
}

impl StoreSort {
    fn as_str(self) -> &'static str {
        match self {
            Self::Popularity => "popularity",
            Self::Newest => "newest",
        }
    }
}

pub fn asset_url(id: i32) -> String {
    format!("{}/asset/{id}", base_url())
}

async fn get_public<T: DeserializeOwned>(
    path: &str,
    query: &[(&str, String)],
) -> Result<T, PlusError> {
    if plus_blocked() {
        return Err(PlusError::ConsentRequired);
    }
    let url = reqwest::Url::parse_with_params(
        &format!("{}{path}", base_url()),
        query.iter().map(|(key, value)| (*key, value.as_str())),
    )?;
    let response = client()?
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(PlusError::Api {
            status: response.status().as_u16(),
            body: response.text().await.unwrap_or_default(),
        });
    }
    Ok(response.json::<T>().await?)
}

pub async fn search_store(
    sort: StoreSort,
    kind: Option<CosmeticType>,
    per_page: u32,
) -> Result<Vec<StoreItem>, PlusError> {
    let mut query = vec![
        ("page", "1".to_string()),
        ("nb", per_page.clamp(1, 100).to_string()),
        ("sort", sort.as_str().to_string()),
    ];
    if let Some(kind) = kind
        && let Ok(serde_json::Value::String(name)) = serde_json::to_value(kind)
    {
        query.push(("types", name));
    }
    Ok(get_public::<StoreSearch>("/cosmetics/search", &query)
        .await?
        .results)
}

pub async fn search_bundles(per_page: u32) -> Result<Vec<BundleInfo>, PlusError> {
    let query = [("nb", per_page.clamp(1, 100).to_string())];
    Ok(get_public::<BundleSearch>("/bundles/search", &query)
        .await?
        .bundles)
}

pub async fn store_product_id(id: i32) -> Result<Option<String>, PlusError> {
    Ok(
        get_public::<StoreView>(&format!("/cosmetics/view/{id}"), &[])
            .await?
            .store_product_id
            .filter(|product| !product.trim().is_empty()),
    )
}

pub async fn create_checkout(
    account: &MinecraftAccount,
    products: &[String],
) -> Result<String, PlusError> {
    let body = serde_json::json!({
        "player": account.id.hyphenated().to_string(),
        "products": products,
    });
    Ok(json_authorized::<CheckoutResponse>(
        account,
        Method::POST,
        "/v1/checkout/create",
        Some(&body),
    )
    .await?
    .url)
}

pub async fn transactions(account: &MinecraftAccount) -> Result<Vec<Transaction>, PlusError> {
    let mut list =
        json_authorized::<Transactions>(account, Method::GET, "/transactions/player", None)
            .await?
            .transactions;
    list.sort_by_key(|entry| std::cmp::Reverse(entry.id));
    Ok(list)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwatchArt {
    Picture(Bytes),
    Stripes(Vec<[u8; 3]>),
}

const CAPE_TEXTURE_WIDTH: f32 = 64.;
const LEGACY_CAPE_TEXTURE_WIDTH: f32 = 22.;
const MAX_STRIPES: usize = 3;
const MIN_STRIPE_SHARE: f32 = 0.08;

pub async fn swatch_art(url: &str, cape: bool) -> Result<SwatchArt, PlusError> {
    let bytes = cape_texture(url).await?;
    tokio::task::spawn_blocking(move || build_swatch(&bytes, cape))
        .await
        .map_err(|err| PlusError::Asset(err.to_string()))?
}

fn build_swatch(bytes: &[u8], cape: bool) -> Result<SwatchArt, PlusError> {
    let image = image::load_from_memory(bytes)
        .map_err(|err| PlusError::Asset(err.to_string()))?
        .to_rgba8();
    if cape {
        let unit = if image.width() as f32 >= CAPE_TEXTURE_WIDTH {
            image.width() as f32 / CAPE_TEXTURE_WIDTH
        } else {
            image.width() as f32 / LEGACY_CAPE_TEXTURE_WIDTH
        };
        let offset = unit as u32;
        let width = (unit * 10.) as u32;
        let height = (unit * 16.) as u32;
        if width == 0
            || height == 0
            || offset + width > image.width()
            || offset + height > image.height()
        {
            return Err(PlusError::Asset("the cape texture is too small".into()));
        }
        let front = image::imageops::crop_imm(&image, offset, offset, width, height).to_image();
        let mut png = std::io::Cursor::new(Vec::new());
        front
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|err| PlusError::Asset(err.to_string()))?;
        return Ok(SwatchArt::Picture(Bytes::from(png.into_inner())));
    }

    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    let mut total = 0u32;
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 128 {
            continue;
        }
        let bucket = [(r & 0xF0) | 0x08, (g & 0xF0) | 0x08, (b & 0xF0) | 0x08];
        *counts.entry(bucket).or_default() += 1;
        total += 1;
    }
    if total == 0 {
        return Err(PlusError::Asset("the texture is empty".into()));
    }
    let mut ranked: Vec<([u8; 3], u32)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let colors: Vec<[u8; 3]> = ranked
        .into_iter()
        .filter(|(_, count)| *count as f32 >= total as f32 * MIN_STRIPE_SHARE)
        .take(MAX_STRIPES)
        .map(|(color, _)| color)
        .collect();
    if colors.is_empty() {
        return Err(PlusError::Asset("no dominant colours".into()));
    }
    Ok(SwatchArt::Stripes(colors))
}

pub fn texture_hash(url: &str) -> Option<String> {
    let hash = url
        .trim_end_matches('/')
        .rsplit('/')
        .next()?
        .to_ascii_lowercase();
    (32..=64)
        .contains(&hash.len())
        .then_some(())
        .filter(|()| hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(|()| hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_player_cosmetics() {
        let json = r#"{
            "cosmetics": [{
                "id": 4, "type": "cape", "name": "Pride", "allowed_slots": ["cape"],
                "variants": [{"id": 9, "name": "Blue", "allowed_slots": ["cape"], "hash": "x",
                    "cover_url": "https://example.com/c.png"}]
            }, {
                "id": 5, "type": "future_thing", "name": "New", "allowed_slots": ["tail"], "variants": []
            }],
            "emotes": [{"id": 30, "name": "Wave", "hash": "y"}],
            "equipped": {"cape": 9},
            "particle_color": null,
            "expiring": {}
        }"#;
        let player: PlayerCosmetics = serde_json::from_str(json).unwrap();
        assert_eq!(player.cosmetics[0].kind, CosmeticType::Cape);
        assert_eq!(player.cosmetics[1].kind, CosmeticType::Unknown);
        assert_eq!(player.cosmetics[1].allowed_slots, vec![BodySlot::Unknown]);
        assert_eq!(player.equipped.get(&BodySlot::Cape), Some(&9));
        assert_eq!(
            player.cosmetics[0].variants[0].cover_url.as_deref(),
            Some("https://example.com/c.png")
        );
    }

    #[test]
    fn serializes_equip_changes() {
        let changes = HashMap::from([(BodySlot::LeftHand, None), (BodySlot::Cape, Some(3))]);
        let body = serde_json::json!({ "equipped": changes });
        assert_eq!(body["equipped"]["cape"], 3);
        assert!(body["equipped"]["left_hand"].is_null());
    }

    #[test]
    fn parses_outfits() {
        let json = r#"{"outfits": [{"id": 1, "name": "Outfit #1", "favorite": false, "skin_id": null,
            "mojang_cape_id": null, "particle_color": null, "blank": true, "equipped": {"hat": 2},
            "created_at": "", "updated_at": ""}], "active_outfit_id": 1, "limit": 128}"#;
        let list: OutfitList = serde_json::from_str(json).unwrap();
        assert!(list.outfits[0].blank);
        assert_eq!(list.outfits[0].equipped.get(&BodySlot::Hat), Some(&2));
    }

    #[test]
    fn builds_cape_front_and_stripes() {
        let mut cape = image::RgbaImage::new(64, 32);
        for (x, y, pixel) in cape.enumerate_pixels_mut() {
            *pixel = if (1..11).contains(&x) && (1..17).contains(&y) {
                image::Rgba([200, 30, 30, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            };
        }
        let mut png = std::io::Cursor::new(Vec::new());
        cape.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let bytes = png.into_inner();

        match build_swatch(&bytes, true).unwrap() {
            SwatchArt::Picture(front) => {
                let decoded = image::load_from_memory(&front).unwrap();
                assert_eq!((decoded.width(), decoded.height()), (10, 16));
            }
            other => panic!("expected a picture, got {other:?}"),
        }
        match build_swatch(&bytes, false).unwrap() {
            SwatchArt::Stripes(colors) => assert_eq!(colors, vec![[0xC8, 0x18, 0x18]]),
            other => panic!("expected stripes, got {other:?}"),
        }
    }

    #[test]
    fn parses_sale_info() {
        let json = r#"{"id": 1, "name": "Cape", "type": "cape", "base_price": 10.0,
            "sale": {"price": 7.5, "discount": 2.5, "percent": 25, "name": "Spring", "ends_at": "2026-12-01"}}"#;
        let item: StoreItem = serde_json::from_str(json).unwrap();
        let sale = item.sale.as_ref().unwrap();
        assert_eq!(sale.percent, 25);
        assert_eq!(sale.name, "Spring");
        assert_eq!(sale.ends_at.as_deref(), Some("2026-12-01"));
        assert_eq!(item.final_price(), Some(7.5));
        assert_eq!(item.discount_percent(), Some(25));
        assert!(item.discounted());

        let json = r#"{"id": 2, "name": "Hat", "type": "hat", "base_price": 4.0}"#;
        let item: StoreItem = serde_json::from_str(json).unwrap();
        assert!(item.sale.is_none());
        assert_eq!(item.final_price(), Some(4.0));
        assert_eq!(item.discount_percent(), None);
        assert!(!item.discounted());
    }

    #[test]
    fn bundle_sale_prices() {
        let json = r#"{"id": 3, "name": "Pack", "base_price": 20.0,
            "sale": {"price": 15.0, "discount": 5.0, "percent": 25}}"#;
        let bundle: BundleInfo = serde_json::from_str(json).unwrap();
        assert_eq!(bundle.final_price(), Some(15.0));
        assert_eq!(bundle.discount_percent(), Some(25));
        assert!(bundle.discounted());

        let json = r#"{"id": 4, "name": "Pack", "base_price": 20.0, "sale": {"price": 20.0, "percent": 0}}"#;
        let bundle: BundleInfo = serde_json::from_str(json).unwrap();
        assert_eq!(bundle.discount_percent(), None);
        assert!(!bundle.discounted());
    }

    #[test]
    fn store_ui_variants_dedupe() {
        let json = r#"{"id": 1, "name": "Cape", "type": "cape", "variants": [
            {"id": 12, "variant_name": "Blue", "variant_order": 2},
            {"id": 10, "variant_name": " Red ", "variant_order": 1, "model_variant": "wide"},
            {"id": 11, "variant_name": "Red", "variant_order": 1, "model_variant": "slim"},
            {"id": 13, "variant_name": "  ", "variant_order": 3}
        ]}"#;
        let item: StoreItem = serde_json::from_str(json).unwrap();
        let ids: Vec<i32> = item.ui_variants().iter().map(|v| v.id).collect();
        assert_eq!(ids, vec![10, 12, 13]);

        let json = r#"{"id": 5, "name": "Solo", "type": "hat"}"#;
        let item: StoreItem = serde_json::from_str(json).unwrap();
        let ids: Vec<i32> = item.ui_variants().iter().map(|v| v.id).collect();
        assert_eq!(ids, vec![5]);
    }

    fn skin_group() -> CosmeticGroup {
        let json = r#"{"id": 1, "type": "hat", "name": "Hat", "variants": [
            {"id": 1, "name": "Red", "model": "wide"},
            {"id": 2, "name": "Red", "model": "slim"},
            {"id": 3, "name": "Blue", "model": "wide"},
            {"id": 4, "name": "Green"},
            {"id": 5, "name": "Green"}
        ]}"#;
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn cosmetic_ui_variants_dedupe() {
        let group = skin_group();
        let ids: Vec<i32> = group.ui_variants().iter().map(|v| v.id).collect();
        assert_eq!(ids, vec![1, 3, 4]);
    }

    #[test]
    fn variant_for_skin_picks_sibling() {
        let group = skin_group();
        assert_eq!(group.variant_for_skin(1, true), 2);
        assert_eq!(group.variant_for_skin(2, false), 1);
        assert_eq!(group.variant_for_skin(1, false), 1);
        assert_eq!(group.variant_for_skin(3, true), 3);
        assert_eq!(group.variant_for_skin(4, true), 4);
        assert_eq!(group.variant_for_skin(99, true), 99);
    }

    #[test]
    fn parses_pet_slot() {
        let slot: BodySlot = serde_json::from_str("\"pet\"").unwrap();
        assert_eq!(slot, BodySlot::Pet);
        assert_eq!(CosmeticType::Pet.default_slot(), Some(BodySlot::Pet));
        assert_eq!(
            CosmeticType::Glove.default_slot(),
            Some(BodySlot::RightHand)
        );
        assert_eq!(CosmeticType::Emote.default_slot(), None);
    }

    #[test]
    fn extracts_texture_hashes() {
        let hash = "1a4af718455d4aab528e7a61f86fa25e6a369d1768dcb13f7df319a713eb810b";
        assert_eq!(
            texture_hash(&format!("https://textures.minecraft.net/texture/{hash}")).as_deref(),
            Some(hash)
        );
        assert_eq!(texture_hash("https://example.com/not-a-hash"), None);
    }
}
