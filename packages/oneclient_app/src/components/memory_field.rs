use freya::prelude::*;

use crate::components::{Dropdown, TextInput, validate_memory};
use crate::theme::colors;
use crate::utils::{format_memory_gb, memory_presets_mb};

const CUSTOM_LABEL: &str = "Custom";

/// Presets picker with an input for memory allocation
pub fn memory_field(
    mut memory: State<String>,
    unset_label: &str,
    placeholder: u32,
) -> impl IntoElement {
    let presets = memory_presets_mb();
    let selected = match memory.read().trim() {
        "" => unset_label.to_string(),
        value => value
            .parse::<u32>()
            .ok()
            .filter(|mb| presets.contains(mb))
            .map(format_memory_gb)
            .unwrap_or_else(|| CUSTOM_LABEL.to_string()),
    };

    let mut options: Vec<String> = vec![unset_label.to_string()];
    options.extend(presets.iter().copied().map(format_memory_gb));

    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(
            Dropdown::new(selected, options)
                .width(Size::px(100.))
                .height(Size::px(34.))
                .on_select(move |idx: usize| {
                    if idx == 0 {
                        memory.set(String::new());
                    } else if let Some(mb) = presets.get(idx - 1).copied() {
                        memory.set(mb.to_string());
                    }
                }),
        )
        .child(
            TextInput::new(memory)
                .width(Size::px(90.))
                .placeholder(placeholder.to_string())
                .on_validate(validate_memory)
                .trailing(
                    label()
                        .text("MB")
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
}
