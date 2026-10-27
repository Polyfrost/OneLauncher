use std::cell::Cell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use freya::prelude::{Color, State};
use serde::{Deserialize, Serialize};

use super::{Theme, resolve_theme};

pub const MODRINTH_COLOR: Color = Color::from_rgb(27, 217, 106);
pub const CURSEFORGE_COLOR: Color = Color::from_rgb(241, 100, 54);

const NO_ACCENT: u32 = u32::MAX;
const GHOST_ALPHA: [u8; 3] = [12, 26, 38];

static ACTIVE_THEME: RwLock<String> = RwLock::new(String::new());
static PREVIEW_THEME: RwLock<Option<Arc<Theme>>> = RwLock::new(None);
static ACTIVE_ACCENT: AtomicU32 = AtomicU32::new(NO_ACCENT);

thread_local! {
    static REVISION: Cell<Option<State<u32>>> = const { Cell::new(None) };
}

type Rgb = (u8, u8, u8);

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Hex(Color);

impl TryFrom<String> for Hex {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let digits = value.trim().trim_start_matches('#');
        let packed = u32::from_str_radix(digits, 16)
            .ok()
            .filter(|_| digits.is_ascii());
        match (digits.len(), packed) {
            (6, Some(packed)) => Ok(Hex(color(unpack(packed)))),
            (8, Some(packed)) => {
                let (r, g, b) = unpack(packed >> 8);
                Ok(Hex(Color::from_argb(packed as u8, r, g, b)))
            }
            _ => Err(format!("'{value}' is not a #RRGGBB or #RRGGBBAA color")),
        }
    }
}

impl From<Hex> for String {
    fn from(Hex(color): Hex) -> Self {
        match color.a() {
            255 => format_hex(color),
            alpha => format!("{}{alpha:02X}", format_hex(color)),
        }
    }
}

impl From<Color> for Hex {
    fn from(color: Color) -> Self {
        Hex(color)
    }
}

impl Hex {
    pub fn color(self) -> Color {
        self.0
    }

    fn rgb(self) -> Rgb {
        (self.0.r(), self.0.g(), self.0.b())
    }
}

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct Palette {
    #[serde(default)]
    pub light: bool,
    pub page: Hex,
    pub page_elevated: Hex,
    pub card: Hex,
    pub component: Hex,
    pub border: Hex,
    pub text: Hex,
    pub text_secondary: Hex,
    pub accent: Hex,
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

fn bump_revision() {
    REVISION.with(|slot| {
        if let Some(mut revision) = slot.get() {
            *revision.write() += 1;
        }
    });
}

pub fn apply(theme: &str, accent: Option<&str>) {
    theme.clone_into(&mut ACTIVE_THEME.write().unwrap());
    let packed = accent.and_then(parse_hex).map_or(NO_ACCENT, pack);
    ACTIVE_ACCENT.store(packed, Ordering::Relaxed);
}

pub fn preview(theme: Option<Theme>) {
    *PREVIEW_THEME.write().unwrap() = theme.map(Arc::new);
    bump_revision();
}

pub fn active_theme() -> Arc<Theme> {
    REVISION.with(|slot| {
        if let Some(revision) = slot.get() {
            let _ = revision.try_read();
        }
    });
    if let Some(preview) = PREVIEW_THEME.read().unwrap().clone() {
        return preview;
    }
    resolve_theme(&ACTIVE_THEME.read().unwrap())
}

fn palette() -> Palette {
    active_theme().colors
}

fn strong_rgb() -> Rgb {
    if is_light() {
        (0, 0, 0)
    } else {
        (255, 255, 255)
    }
}

fn brand_rgb() -> Rgb {
    match ACTIVE_ACCENT.load(Ordering::Relaxed) {
        NO_ACCENT => palette().accent.rgb(),
        packed => unpack(packed),
    }
}

pub fn page() -> Color {
    palette().page.color()
}

pub fn page_elevated() -> Color {
    palette().page_elevated.color()
}

pub fn card() -> Color {
    palette().card.color()
}

pub fn fg_primary() -> Color {
    palette().text.color()
}

pub fn fg_primary_hover() -> Color {
    color(mix(palette().text.rgb(), strong_rgb(), 0.12))
}

pub fn fg_primary_pressed() -> Color {
    color(mix(palette().text.rgb(), strong_rgb(), 0.25))
}

pub fn fg_primary_disabled() -> Color {
    let palette = palette();
    color(mix(palette.text.rgb(), palette.page.rgb(), 0.3))
}

pub fn fg_secondary() -> Color {
    palette().text_secondary.color()
}

pub fn fg_secondary_hover() -> Color {
    color(mix(palette().text_secondary.rgb(), (0, 0, 0), 0.2))
}

pub fn fg_secondary_pressed() -> Color {
    color(mix(palette().text_secondary.rgb(), (0, 0, 0), 0.4))
}

pub fn fg_strong() -> Color {
    color(strong_rgb())
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
    mix(brand_rgb(), palette().page.rgb(), 0.5)
}

pub fn brand_disabled() -> Color {
    color(brand_disabled_rgb())
}

pub fn disabled_opacity() -> f32 {
    if is_light() { 0.75 } else { 0.5 }
}

fn tinted(alpha: u8) -> Color {
    let (r, g, b) = strong_rgb();
    Color::from_argb(alpha, r, g, b)
}

pub fn ghost_overlay() -> Color {
    tinted(GHOST_ALPHA[0])
}

pub fn ghost_overlay_hover() -> Color {
    tinted(GHOST_ALPHA[1])
}

pub fn ghost_overlay_pressed() -> Color {
    tinted(GHOST_ALPHA[2])
}

pub fn component_bg() -> Color {
    palette().component.color()
}

pub fn component_bg_hover() -> Color {
    color(mix(palette().component.rgb(), strong_rgb(), 0.04))
}

pub fn component_bg_pressed() -> Color {
    color(mix(palette().component.rgb(), strong_rgb(), 0.09))
}

pub fn component_bg_disabled() -> Color {
    let palette = palette();
    color(mix(palette.component.rgb(), palette.page.rgb(), 0.5))
}

fn border_scaled(scale: f32) -> Color {
    let border = palette().border.color();
    border.with_a((f32::from(border.a()) * scale).min(255.) as u8)
}

pub fn component_border() -> Color {
    border_scaled(1.)
}

pub fn component_border_hover() -> Color {
    border_scaled(1.6)
}

pub fn component_border_pressed() -> Color {
    border_scaled(2.2)
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
