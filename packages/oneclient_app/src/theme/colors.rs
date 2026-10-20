use std::cell::Cell;
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

use freya::prelude::{Color, State};
use oneclient_core::settings::AppTheme;

pub const MODRINTH_COLOR: Color = Color::from_rgb(27, 217, 106);
pub const CURSEFORGE_COLOR: Color = Color::from_rgb(241, 100, 54);

const NO_ACCENT: u32 = u32::MAX;

static ACTIVE_THEME: AtomicU8 = AtomicU8::new(0);
static ACTIVE_ACCENT: AtomicU32 = AtomicU32::new(NO_ACCENT);

thread_local! {
    static REVISION: Cell<Option<State<u32>>> = const { Cell::new(None) };
}

type Rgb = (u8, u8, u8);

struct Palette {
    page: Rgb,
    page_elevated: Rgb,
    card: Rgb,
    fg_primary: [Rgb; 4],
    fg_strong: Rgb,
    fg_secondary: [Rgb; 3],
    component_bg: [Rgb; 4],
    brand: Rgb,
    tint: Rgb,
    ghost_alpha: [u8; 3],
    border_alpha: [u8; 3],
}

const DARK: Palette = Palette {
    page: (17, 23, 28),
    page_elevated: (21, 28, 34),
    card: (26, 34, 41),
    fg_primary: [
        (213, 219, 255),
        (218, 224, 255),
        (225, 229, 255),
        (205, 209, 235),
    ],
    fg_strong: (255, 255, 255),
    fg_secondary: [(120, 129, 141), (95, 104, 116), (75, 84, 96)],
    component_bg: [(26, 34, 40), (29, 36, 43), (34, 44, 53), (16, 24, 31)],
    brand: (43, 75, 255),
    tint: (255, 255, 255),
    ghost_alpha: [12, 26, 38],
    border_alpha: [12, 25, 38],
};

const AMOLED: Palette = Palette {
    page: (0, 0, 0),
    page_elevated: (9, 10, 12),
    card: (14, 15, 18),
    fg_primary: [
        (213, 219, 255),
        (218, 224, 255),
        (225, 229, 255),
        (205, 209, 235),
    ],
    fg_strong: (255, 255, 255),
    fg_secondary: [(124, 129, 140), (98, 103, 114), (78, 83, 94)],
    component_bg: [(16, 18, 21), (22, 24, 28), (30, 33, 38), (8, 9, 11)],
    brand: (111, 143, 255),
    tint: (255, 255, 255),
    ghost_alpha: [12, 26, 38],
    border_alpha: [14, 28, 40],
};

const GREY: Palette = Palette {
    page: (40, 42, 54),
    page_elevated: (47, 49, 63),
    card: (53, 56, 71),
    fg_primary: [
        (236, 237, 248),
        (241, 242, 250),
        (246, 247, 252),
        (205, 207, 222),
    ],
    fg_strong: (255, 255, 255),
    fg_secondary: [(143, 146, 168), (118, 121, 144), (96, 99, 122)],
    component_bg: [(54, 57, 73), (60, 63, 80), (68, 71, 90), (44, 46, 59)],
    brand: (121, 112, 169),
    tint: (255, 255, 255),
    ghost_alpha: [12, 26, 38],
    border_alpha: [14, 28, 40],
};

const LIGHT: Palette = Palette {
    page: (244, 250, 255),
    page_elevated: (233, 241, 249),
    card: (255, 255, 255),
    fg_primary: [(22, 30, 54), (12, 20, 42), (4, 10, 30), (92, 100, 124)],
    fg_strong: (8, 14, 36),
    fg_secondary: [(94, 106, 128), (74, 86, 108), (54, 66, 88)],
    component_bg: [
        (224, 234, 245),
        (214, 226, 239),
        (202, 216, 232),
        (236, 243, 250),
    ],
    brand: (57, 87, 255),
    tint: (0, 0, 0),
    ghost_alpha: [14, 28, 42],
    border_alpha: [26, 44, 62],
};

const WHITE: Palette = Palette {
    page: (255, 255, 255),
    page_elevated: (246, 247, 250),
    card: (255, 255, 255),
    fg_primary: [(24, 26, 38), (14, 16, 28), (6, 8, 20), (100, 104, 122)],
    fg_strong: (10, 12, 24),
    fg_secondary: [(104, 108, 126), (84, 88, 106), (64, 68, 86)],
    component_bg: [
        (238, 240, 245),
        (229, 232, 239),
        (218, 222, 232),
        (246, 247, 250),
    ],
    brand: (192, 195, 210),
    tint: (0, 0, 0),
    ghost_alpha: [12, 26, 38],
    border_alpha: [24, 42, 60],
};

fn palette_for(theme: AppTheme) -> &'static Palette {
    match theme {
        AppTheme::Dark => &DARK,
        AppTheme::Amoled => &AMOLED,
        AppTheme::Grey => &GREY,
        AppTheme::Light => &LIGHT,
        AppTheme::White => &WHITE,
    }
}

fn theme_from_index(index: u8) -> AppTheme {
    AppTheme::ALL
        .get(usize::from(index))
        .copied()
        .unwrap_or_default()
}

fn theme_index(theme: AppTheme) -> u8 {
    AppTheme::ALL
        .iter()
        .position(|candidate| *candidate == theme)
        .unwrap_or_default() as u8
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

pub fn apply(theme: AppTheme, accent: Option<&str>) -> bool {
    let index = theme_index(theme);
    let packed = accent.and_then(parse_hex).map_or(NO_ACCENT, pack);
    let previous_theme = ACTIVE_THEME.swap(index, Ordering::Relaxed);
    let previous_accent = ACTIVE_ACCENT.swap(packed, Ordering::Relaxed);
    previous_theme != index || previous_accent != packed
}

pub fn active_theme() -> AppTheme {
    theme_from_index(ACTIVE_THEME.load(Ordering::Relaxed))
}

pub fn theme_brand(theme: AppTheme) -> Color {
    color(palette_for(theme).brand)
}

fn palette() -> &'static Palette {
    REVISION.with(|slot| {
        if let Some(revision) = slot.get() {
            let _ = revision.try_read();
        }
    });
    palette_for(active_theme())
}

fn brand_rgb() -> Rgb {
    let palette = palette();
    match ACTIVE_ACCENT.load(Ordering::Relaxed) {
        NO_ACCENT => palette.brand,
        packed => unpack(packed),
    }
}

pub fn page() -> Color {
    color(palette().page)
}

pub fn page_elevated() -> Color {
    color(palette().page_elevated)
}

pub fn card() -> Color {
    color(palette().card)
}

pub fn fg_primary() -> Color {
    color(palette().fg_primary[0])
}

pub fn fg_primary_hover() -> Color {
    color(palette().fg_primary[1])
}

pub fn fg_primary_pressed() -> Color {
    color(palette().fg_primary[2])
}

pub fn fg_primary_disabled() -> Color {
    color(palette().fg_primary[3])
}

pub fn fg_secondary() -> Color {
    color(palette().fg_secondary[0])
}

pub fn fg_secondary_hover() -> Color {
    color(palette().fg_secondary[1])
}

pub fn fg_secondary_pressed() -> Color {
    color(palette().fg_secondary[2])
}

pub fn fg_strong() -> Color {
    color(palette().fg_strong)
}

pub fn on_media() -> Color {
    Color::from_rgb(213, 219, 255)
}

pub fn shade() -> Color {
    let palette = palette();
    if palette.tint == (0, 0, 0) {
        color(palette.page)
    } else {
        Color::BLACK
    }
}

pub fn on_brand() -> Color {
    Color::from_rgb(213, 219, 255)
}

pub fn on_brand_disabled() -> Color {
    Color::from_rgb(205, 209, 235)
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

pub fn brand_disabled() -> Color {
    color(mix(brand_rgb(), palette().page, 0.5))
}

fn tinted(alpha: u8) -> Color {
    let (r, g, b) = palette().tint;
    Color::from_argb(alpha, r, g, b)
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
    color(palette().component_bg[0])
}

pub fn component_bg_hover() -> Color {
    color(palette().component_bg[1])
}

pub fn component_bg_pressed() -> Color {
    color(palette().component_bg[2])
}

pub fn component_bg_disabled() -> Color {
    color(palette().component_bg[3])
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

pub fn code_info() -> Color {
    Color::from_rgb(97, 175, 239)
}

pub fn code_warn() -> Color {
    Color::from_rgb(229, 192, 123)
}

pub fn code_error() -> Color {
    Color::from_rgb(224, 108, 117)
}

pub fn code_debug() -> Color {
    Color::from_rgb(152, 195, 121)
}

pub fn code_chat() -> Color {
    Color::from_rgb(198, 120, 221)
}

pub fn code_muted() -> Color {
    Color::from_rgb(120, 128, 140)
}

pub fn selection_bg() -> Color {
    Color::from_rgb(97, 175, 239).with_a(60)
}

pub fn toast_action() -> Color {
    Color::from_rgb(155, 161, 166)
}
