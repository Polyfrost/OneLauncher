use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use bytes::Bytes;
use freya::prelude::CornerRadius;
use serde::Deserialize;

use crate::AppAssets;
use colors::Palette;

pub mod colors;

pub const NAVBAR_HEIGHT_PX: f32 = 80.;
pub const HOME_PADDING_PX: f32 = 48.;
pub const DEFAULT_FONT: &str = "Poppins";
pub const MONO_FONT: &str = "JetBrains Mono";

const BUILTIN_THEMES: [&str; 5] = ["dark", "amoled", "grey", "light", "white"];

static THEMES: LazyLock<Vec<Theme>> = LazyLock::new(load_themes);

#[derive(Deserialize)]
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
    #[serde(default)]
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

pub fn themes() -> &'static [Theme] {
    &THEMES
}

fn parse_theme(id: &str, json: &[u8], dir: Option<&Path>) -> Option<Theme> {
    match serde_json::from_slice::<Theme>(json) {
        Ok(mut theme) => {
            theme.id = id.to_string();
            theme.dir = dir.map(Path::to_path_buf);
            Some(theme)
        }
        Err(err) => {
            tracing::error!("Failed to load theme '{id}': {err}");
            None
        }
    }
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

pub fn font() -> &'static str {
    &colors::active_theme().font
}

pub fn mono_font() -> &'static str {
    &colors::active_theme().mono_font
}

pub fn radius(radius: f32) -> CornerRadius {
    CornerRadius::new_all(radius * colors::active_theme().radius_scale)
}

pub fn load_fonts() -> Vec<(&'static str, Bytes)> {
    const FONTS: [(&str, &str); 11] = [
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
    ];

    let embedded = FONTS.iter().filter_map(|&(weight, file)| {
        let bytes = AppAssets::get_bytes(file).or_else(|| {
            tracing::error!("Failed to load embedded font '{file}'");
            None
        })?;

        Some((weight, bytes))
    });

    let from_themes = themes().iter().flat_map(|theme| {
        theme.font_files.iter().flat_map(move |(family, files)| {
            files.iter().filter_map(move |file| {
                let path = theme.dir.as_deref()?.join(file);
                let bytes = std::fs::read(&path)
                    .inspect_err(|err| {
                        tracing::error!("Failed to load theme font '{}': {err}", path.display())
                    })
                    .ok()?;

                Some((family.as_str(), Bytes::from(bytes)))
            })
        })
    });

    embedded.chain(from_themes).collect()
}
