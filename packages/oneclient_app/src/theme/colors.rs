use std::cell::Cell;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use freya::prelude::{Color, State};
use serde::Deserialize;

use super::{Theme, themes};

pub const MODRINTH_COLOR: Color = Color::from_rgb(27, 217, 106);
pub const CURSEFORGE_COLOR: Color = Color::from_rgb(241, 100, 54);

const NO_ACCENT: u32 = u32::MAX;

static ACTIVE_THEME: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_ACCENT: AtomicU32 = AtomicU32::new(NO_ACCENT);

thread_local! {
    static REVISION: Cell<Option<State<u32>>> = const { Cell::new(None) };
}

type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
struct Hex(Rgb);

impl TryFrom<String> for Hex {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        parse_hex(&value)
            .map(Hex)
            .ok_or_else(|| format!("'{value}' is not a #RRGGBB color"))
    }
}

#[derive(Deserialize)]
pub struct Palette {
    #[serde(default)]
    light: bool,
    page: Hex,
    page_elevated: Hex,
    card: Hex,
    fg_primary: [Hex; 4],
    fg_strong: Hex,
    fg_secondary: [Hex; 3],
    component_bg: [Hex; 4],
    brand: Hex,
    ghost_alpha: [u8; 3],
    border_alpha: [u8; 3],
}

impl Palette {
    pub fn page(&self) -> Color {
        color(self.page.0)
    }

    pub fn brand(&self) -> Color {
        color(self.brand.0)
    }

    pub fn is_light(&self) -> bool {
        self.light
    }
}

pub fn parse_hex(value: &str) -> Option<Rgb> {
    let digits = value.trim().trim_start_matches('#');
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let packed = u32::from_str_radix(digits, 16).ok()?;
    Some(unpack(packed))
}

pub fn format_hex(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

fn unpack(packed: u32) -> Rgb {
    ((packed >> 16) as u8, (packed >> 8) as u8, packed as u8)
}

fn pack((r, g, b): Rgb) -> u32 {
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

fn color((r, g, b): Rgb) -> Color {
    Color::from_rgb(r, g, b)
}

fn mix((ar, ag, ab): Rgb, (br, bg, bb): Rgb, amount: f32) -> Rgb {
    let blend = |from: u8, to: u8| {
        (f32::from(from) + (f32::from(to) - f32::from(from)) * amount).round() as u8
    };
    (blend(ar, br), blend(ag, bg), blend(ab, bb))
}

pub fn bind_revision(revision: State<u32>) {
    REVISION.with(|slot| slot.set(Some(revision)));
}

pub fn apply(theme: &str, accent: Option<&str>) -> bool {
    let index = themes()
        .iter()
        .position(|candidate| candidate.id == theme)
        .unwrap_or_default();
    let packed = accent.and_then(parse_hex).map_or(NO_ACCENT, pack);
    let previous_theme = ACTIVE_THEME.swap(index, Ordering::Relaxed);
    let previous_accent = ACTIVE_ACCENT.swap(packed, Ordering::Relaxed);
    previous_theme != index || previous_accent != packed
}

pub fn active_theme() -> &'static Theme {
    REVISION.with(|slot| {
        if let Some(revision) = slot.get() {
            let _ = revision.try_read();
        }
    });
    &themes()[ACTIVE_THEME.load(Ordering::Relaxed)]
}

fn palette() -> &'static Palette {
    &active_theme().colors
}

fn brand_rgb() -> Rgb {
    let palette = palette();
    match ACTIVE_ACCENT.load(Ordering::Relaxed) {
        NO_ACCENT => palette.brand.0,
        packed => unpack(packed),
    }
}

pub fn page() -> Color {
    color(palette().page.0)
}

pub fn page_elevated() -> Color {
    color(palette().page_elevated.0)
}

pub fn card() -> Color {
    color(palette().card.0)
}

pub fn fg_primary() -> Color {
    color(palette().fg_primary[0].0)
}

pub fn fg_primary_hover() -> Color {
    color(palette().fg_primary[1].0)
}

pub fn fg_primary_pressed() -> Color {
    color(palette().fg_primary[2].0)
}

pub fn fg_primary_disabled() -> Color {
    color(palette().fg_primary[3].0)
}

pub fn fg_secondary() -> Color {
    color(palette().fg_secondary[0].0)
}

pub fn fg_secondary_hover() -> Color {
    color(palette().fg_secondary[1].0)
}

pub fn fg_secondary_pressed() -> Color {
    color(palette().fg_secondary[2].0)
}

pub fn fg_strong() -> Color {
    color(palette().fg_strong.0)
}

pub fn on_media() -> Color {
    Color::from_rgb(213, 219, 255)
}

pub fn is_light() -> bool {
    palette().light
}

pub fn on_art() -> Color {
    if is_light() { on_media() } else { fg_primary() }
}

pub fn on_art_secondary() -> Color {
    if is_light() {
        on_media().with_a(190)
    } else {
        fg_secondary()
    }
}

pub fn art_backdrop() -> Color {
    if is_light() { Color::BLACK } else { page() }
}

pub fn shade() -> Color {
    if is_light() { page() } else { Color::BLACK }
}

const ON_LIGHT_BG: Rgb = (22, 30, 54);
const ON_DARK_BG: Rgb = (213, 219, 255);
const ON_DARK_BG_DISABLED: Rgb = (205, 209, 235);

const MIN_LIGHT_TEXT_CONTRAST: f32 = 3.;

fn luminance((r, g, b): Rgb) -> f32 {
    let linear = |channel: u8| {
        let value = f32::from(channel) / 255.;
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn needs_dark_text(background: Rgb) -> bool {
    contrast(ON_DARK_BG, background) < MIN_LIGHT_TEXT_CONTRAST
}

pub fn on_color(background: Color) -> Color {
    let rgb = (background.r(), background.g(), background.b());
    color(if needs_dark_text(rgb) {
        ON_LIGHT_BG
    } else {
        ON_DARK_BG
    })
}

pub fn on_brand() -> Color {
    let brand = brand_rgb();
    color(if needs_dark_text(brand) {
        ON_LIGHT_BG
    } else {
        ON_DARK_BG
    })
}

pub fn on_brand_strong() -> Color {
    if needs_dark_text(brand_rgb()) {
        color(ON_LIGHT_BG)
    } else {
        Color::WHITE
    }
}

pub fn on_brand_disabled() -> Color {
    let background = brand_disabled_rgb();
    if needs_dark_text(background) {
        color(mix(ON_LIGHT_BG, background, 0.35))
    } else {
        color(ON_DARK_BG_DISABLED)
    }
}

pub fn brand() -> Color {
    color(brand_rgb())
}

pub fn brand_hover() -> Color {
    color(mix(brand_rgb(), (0, 0, 0), 0.13))
}

pub fn brand_pressed() -> Color {
    color(mix(brand_rgb(), (255, 255, 255), 0.06))
}

fn brand_disabled_rgb() -> Rgb {
    mix(brand_rgb(), palette().page.0, 0.5)
}

pub fn brand_disabled() -> Color {
    color(brand_disabled_rgb())
}

pub fn disabled_opacity() -> f32 {
    if is_light() { 0.75 } else { 0.5 }
}

fn tinted(alpha: u8) -> Color {
    let tint = if is_light() { 0 } else { 255 };
    Color::from_argb(alpha, tint, tint, tint)
}

pub fn ghost_overlay() -> Color {
    tinted(palette().ghost_alpha[0])
}

pub fn ghost_overlay_hover() -> Color {
    tinted(palette().ghost_alpha[1])
}

pub fn ghost_overlay_pressed() -> Color {
    tinted(palette().ghost_alpha[2])
}

pub fn component_bg() -> Color {
    color(palette().component_bg[0].0)
}

pub fn component_bg_hover() -> Color {
    color(palette().component_bg[1].0)
}

pub fn component_bg_pressed() -> Color {
    color(palette().component_bg[2].0)
}

pub fn component_bg_disabled() -> Color {
    color(palette().component_bg[3].0)
}

pub fn component_border() -> Color {
    tinted(palette().border_alpha[0])
}

pub fn component_border_hover() -> Color {
    tinted(palette().border_alpha[1])
}

pub fn component_border_pressed() -> Color {
    tinted(palette().border_alpha[2])
}

pub fn control_off() -> Color {
    if is_light() {
        component_bg_pressed()
    } else {
        component_bg()
    }
}

pub fn control_border() -> Color {
    if is_light() {
        component_border_pressed()
    } else {
        component_border()
    }
}

pub fn danger() -> Color {
    Color::from_rgb(255, 68, 68)
}

pub fn danger_hover() -> Color {
    Color::from_rgb(214, 52, 52)
}

pub fn danger_pressed() -> Color {
    Color::from_rgb(255, 86, 86)
}

pub fn danger_disabled() -> Color {
    Color::from_rgb(235, 48, 48)
}

pub fn success() -> Color {
    Color::from_rgb(35, 154, 96)
}

fn code(dark: Rgb, light: Rgb) -> Color {
    color(if is_light() { light } else { dark })
}

pub fn code_info() -> Color {
    code((97, 175, 239), (30, 100, 200))
}

pub fn code_warn() -> Color {
    code((229, 192, 123), (150, 95, 0))
}

pub fn code_error() -> Color {
    code((224, 108, 117), (200, 45, 55))
}

pub fn code_debug() -> Color {
    code((152, 195, 121), (55, 125, 45))
}

pub fn code_chat() -> Color {
    code((198, 120, 221), (150, 50, 170))
}

pub fn code_muted() -> Color {
    code((120, 128, 140), (95, 104, 118))
}

pub fn selection_bg() -> Color {
    Color::from_rgb(97, 175, 239).with_a(60)
}

pub fn toast_action() -> Color {
    Color::from_rgb(155, 161, 166)
}
