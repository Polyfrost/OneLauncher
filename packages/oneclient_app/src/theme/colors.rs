use freya::prelude::Color;

pub const MODRINTH_COLOR: Color = Color::from_rgb(27, 217, 106);
pub const CURSEFORGE_COLOR: Color = Color::from_rgb(241, 100, 54);

pub fn page() -> Color {
    Color::from_rgb(17, 23, 28)
}

pub fn page_elevated() -> Color {
    Color::from_rgb(21, 28, 34)
}

pub fn page_overlay() -> Color {
    Color::from_argb(140, 17, 23, 28)
}

pub fn fg_primary() -> Color {
    Color::from_rgb(213, 219, 255)
}

pub fn fg_primary_hover() -> Color {
    Color::from_rgb(218, 224, 255)
}

pub fn fg_primary_pressed() -> Color {
    Color::from_rgb(225, 229, 255)
}

pub fn fg_primary_disabled() -> Color {
    Color::from_rgb(205, 209, 235)
}

pub fn fg_secondary() -> Color {
    Color::from_rgb(120, 129, 141)
}

pub fn fg_secondary_hover() -> Color {
    Color::from_rgb(95, 104, 116)
}

pub fn fg_secondary_pressed() -> Color {
    Color::from_rgb(75, 84, 96)
}

pub fn brand() -> Color {
    Color::from_rgb(43, 75, 255)
}

pub fn brand_hover() -> Color {
    Color::from_rgb(40, 67, 221)
}

pub fn brand_pressed() -> Color {
    Color::from_rgb(57, 87, 255)
}

pub fn brand_disabled() -> Color {
    Color::from_rgb(31, 47, 129)
}

pub fn ghost_overlay() -> Color {
    Color::from_argb(12, 255, 255, 255)
}

pub fn ghost_overlay_hover() -> Color {
    Color::from_argb(26, 255, 255, 255)
}

pub fn ghost_overlay_pressed() -> Color {
    Color::from_argb(38, 255, 255, 255)
}

pub fn component_bg() -> Color {
    Color::from_rgb(26, 34, 40)
}

pub fn component_bg_hover() -> Color {
    Color::from_rgb(29, 36, 43)
}

pub fn component_bg_pressed() -> Color {
    Color::from_rgb(34, 44, 53)
}

pub fn component_bg_disabled() -> Color {
    Color::from_rgb(16, 24, 31)
}

pub fn component_border() -> Color {
    Color::from_argb(12, 255, 255, 255)
}

pub fn component_border_hover() -> Color {
    Color::from_argb(25, 255, 255, 255)
}

pub fn component_border_pressed() -> Color {
    Color::from_argb(38, 255, 255, 255)
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
