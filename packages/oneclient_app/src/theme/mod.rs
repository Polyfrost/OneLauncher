use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use bytes::Bytes;
use freya::prelude::CornerRadius;
use serde::{Deserialize, Serialize};

use crate::AppAssets;
use colors::Palette;

pub mod colors;

pub const NAVBAR_HEIGHT_PX: f32 = 80.;
pub const HOME_PADDING_PX: f32 = 48.;
pub const DEFAULT_FONT: &str = "Poppins";
pub const MONO_FONT: &str = "JetBrains Mono";
const MINECRAFT_FONT: &str = "Minecraft";

const BUILTIN_THEMES: [&str; 4] = ["dark", "light", "minecraft-dark", "minecraft-light"];

thread_local! {
    static THEMES: RefCell<Vec<Rc<Theme>>> =
        RefCell::new(load_themes().into_iter().map(Rc::new).collect());
    static SCALE_FACTOR: Cell<f32> = const { Cell::new(1.) };
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub struct Theme {
    #[serde(skip)]
    pub id: String,
    pub name: String,
    #[serde(default = "default_radius_scale")]
    pub radius_scale: f32,
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default = "default_mono_font")]
    pub mono_font: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixel_font_size: Option<f32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pixel_icons: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub font_files: BTreeMap<String, Vec<PathBuf>>,
    pub colors: Palette,
    #[serde(skip)]
    dir: Option<PathBuf>,
}

fn default_radius_scale() -> f32 {
    1.
}

fn default_font() -> String {
    DEFAULT_FONT.to_string()
}

fn default_mono_font() -> String {
    MONO_FONT.to_string()
}

impl Theme {
    pub fn is_builtin(&self) -> bool {
        self.dir.is_none()
    }
}

pub fn themes() -> Vec<Rc<Theme>> {
    THEMES.with_borrow(Clone::clone)
}

pub const SYSTEM_THEME: &str = "system";

pub fn system_aware(id: &str, system_dark: bool) -> &str {
    match id {
        SYSTEM_THEME if system_dark => "dark",
        SYSTEM_THEME => "light",
        id => id,
    }
}

pub fn resolve_theme(id: &str) -> Rc<Theme> {
    let id = match id {
        "amoled" | "grey" => "dark",
        "white" => "light",
        id => id,
    };
    THEMES.with_borrow(|themes| {
        themes
            .iter()
            .find(|theme| theme.id == id)
            .unwrap_or(&themes[0])
            .clone()
    })
}

pub async fn save_theme(mut theme: Theme) -> anyhow::Result<String> {
    let dir = oneclient_common::paths::themes_dir()?;

    if theme.is_builtin() {
        let slug: String = theme
            .name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let slug = slug.trim_matches('-');
        let base = if slug.is_empty() { "custom" } else { slug };
        let taken = |id: &str| THEMES.with_borrow(|themes| themes.iter().any(|t| t.id == id));
        theme.id = (1..)
            .map(|n| {
                if n == 1 {
                    base.to_string()
                } else {
                    format!("{base}-{n}")
                }
            })
            .find(|id| !taken(id))
            .unwrap_or_default();
        theme.dir = Some(dir.clone());
    }

    let json = serde_json::to_vec_pretty(&theme)?;
    polyio::create_dir_all(&dir).await?;
    polyio::write(dir.join(format!("{}.json", theme.id)), json).await?;

    let id = theme.id.clone();
    THEMES.with_borrow_mut(
        |themes| match themes.iter_mut().find(|existing| existing.id == id) {
            Some(existing) => *existing = Rc::new(theme),
            None => themes.push(Rc::new(theme)),
        },
    );
    colors::refresh_active();
    Ok(id)
}

pub fn pixel_font_size_of(family: &str) -> Option<f32> {
    themes()
        .iter()
        .filter(|theme| theme.font == family)
        .find_map(|theme| theme.pixel_font_size)
}

pub fn font_families() -> Vec<String> {
    let mut families: Vec<String> = [DEFAULT_FONT, MONO_FONT, MINECRAFT_FONT]
        .into_iter()
        .map(str::to_string)
        .chain(
            themes()
                .iter()
                .flat_map(|theme| theme.font_files.keys().cloned().collect::<Vec<_>>()),
        )
        .collect();
    families.sort();
    families.dedup();
    families
}

fn parse_theme(id: &str, json: &[u8], dir: Option<&Path>) -> Option<Theme> {
    let mut theme = serde_json::from_slice::<Theme>(json)
        .inspect_err(|err| tracing::error!("Failed to load theme '{id}': {err}"))
        .ok()?;
    theme.id = id.to_string();
    theme.dir = dir.map(Path::to_path_buf);
    Some(theme)
}

fn load_themes() -> Vec<Theme> {
    let mut themes: Vec<Theme> = BUILTIN_THEMES
        .iter()
        .filter_map(|id| {
            let json = AppAssets::get_bytes(&format!("themes/{id}.json"))?;
            parse_theme(id, &json, None)
        })
        .collect();
    assert!(!themes.is_empty(), "built-in themes failed to load");

    let Ok(dir) = oneclient_common::paths::themes_dir() else {
        return themes;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return themes;
    };

    let mut custom: Vec<Theme> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let id = path.file_stem()?.to_str()?;
            let json = std::fs::read(&path)
                .inspect_err(|err| tracing::error!("Failed to read theme '{id}': {err}"))
                .ok()?;
            parse_theme(id, &json, Some(&dir))
        })
        .collect();
    custom.sort_by(|a, b| a.name.cmp(&b.name));

    for theme in custom {
        match themes.iter_mut().find(|existing| existing.id == theme.id) {
            Some(existing) => *existing = theme,
            None => themes.push(theme),
        }
    }

    themes
}

pub fn font() -> String {
    colors::active_theme().font.clone()
}

pub fn mono_font() -> String {
    colors::active_theme().mono_font.clone()
}

pub fn set_scale_factor(scale: f32) {
    SCALE_FACTOR.set(scale);
}

pub fn text_size(size: f32) -> f32 {
    let Some(step) = colors::active_theme()
        .pixel_font_size
        .filter(|step| *step > 0.)
    else {
        return size;
    };
    let scale = SCALE_FACTOR.get().max(1.);
    (size * scale / step).round().max(1.) * step / scale
}

const PIXEL_ICON_GRID: f32 = 12.;

pub fn pixel_icon(path: &str) -> Option<String> {
    if !colors::active_theme().pixel_icons {
        return None;
    }
    let pixel = format!("icons/pixel/{}", path.strip_prefix("icons/")?);
    AppAssets::get(&pixel).map(|_| pixel)
}

pub fn pixel_icon_size(size: f32) -> f32 {
    let scale = SCALE_FACTOR.get().max(1.);
    (size * scale / PIXEL_ICON_GRID).round().max(1.) * PIXEL_ICON_GRID / scale
}

pub fn rounded_px(radius: f32) -> f32 {
    radius * colors::active_theme().radius_scale
}

pub fn rounded(radius: f32) -> CornerRadius {
    CornerRadius::new_all(rounded_px(radius))
}

pub fn load_fonts() -> Vec<(Cow<'static, str>, Bytes)> {
    const FONTS: [(&str, &str); 13] = [
        (DEFAULT_FONT, "fonts/Poppins/Poppins-Bold.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-BoldItalic.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-SemiBold.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-SemiBoldItalic.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-Medium.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-MediumItalic.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-Regular.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-Italic.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-Light.ttf"),
        (DEFAULT_FONT, "fonts/Poppins/Poppins-LightItalic.ttf"),
        (MONO_FONT, "fonts/JetBrainsMono/JetBrainsMonoVariable.ttf"),
        (MINECRAFT_FONT, "fonts/Minecraft/Minecraft-Regular.otf"),
        (MINECRAFT_FONT, "fonts/Minecraft/Minecraft-Bold.otf"),
    ];

    let mut fonts: Vec<(Cow<'static, str>, Bytes)> = FONTS
        .iter()
        .filter_map(|&(family, file)| {
            let bytes = AppAssets::get_bytes(file).or_else(|| {
                tracing::error!("Failed to load embedded font '{file}'");
                None
            })?;

            Some((Cow::Borrowed(family), bytes))
        })
        .collect();

    for theme in themes() {
        for (family, files) in &theme.font_files {
            for file in files {
                let Some(path) = theme.dir.as_deref().map(|dir| dir.join(file)) else {
                    continue;
                };
                match std::fs::read(&path) {
                    Ok(bytes) => fonts.push((Cow::Owned(family.clone()), Bytes::from(bytes))),
                    Err(err) => {
                        tracing::error!("Failed to load theme font '{}': {err}", path.display())
                    }
                }
            }
        }
    }

    fonts
}
