use bytes::Bytes;

use crate::AppAssets;

pub mod colors;

pub const NAVBAR_HEIGHT_PX: f32 = 80.;
pub const DEFAULT_FONT: &str = "Poppins";
pub const MONO_FONT: &str = "JetBrains Mono";

pub fn load_fonts() -> Vec<(&'static str, Bytes)> {
    const FONTS: [(&str, &str); 11] = [
        // UI Font
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
        // Mono Font
        (MONO_FONT, "fonts/JetBrainsMono/JetBrainsMonoVariable.ttf"),
    ];

    FONTS
        .iter()
        .filter_map(|&(weight, file)| {
            let bytes = AppAssets::get_bytes(file).or_else(|| {
                tracing::error!("Failed to load embedded font '{file}'");
                None
            })?;

            Some((weight, bytes))
        })
        .collect()
}
