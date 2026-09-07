use freya::prelude::*;

use crate::components::{Dropdown, TextInput, validate_memory};
use crate::theme::colors;
use crate::utils::{format_memory_gb, memory_presets_mb};

const UNSET_LABEL: &str = "Default";
const CUSTOM_LABEL: &str = "Custom";

/// Presets picker with an input for memory allocation
pub fn memory_field(mut memory: State<String>) -> impl IntoElement {
    let presets = memory_presets_mb();
    let selected = match memory.read().trim() {
        "" => UNSET_LABEL.to_string(),
        value => value
            .parse::<u32>()
            .ok()
            .filter(|mb| presets.contains(mb))
            .map(format_memory_gb)
            .unwrap_or_else(|| CUSTOM_LABEL.to_string()),
    };

    let options: Vec<String> = presets.iter().copied().map(format_memory_gb).collect();

    rect()
        .horizontal()
        .cross_align(Alignment::Center)
        .spacing(8.)
        .child(
            Dropdown::new(selected, options)
                .width(Size::px(100.))
                .height(Size::px(34.))
                .on_select(move |idx: usize| {
                    if let Some(mb) = presets.get(idx).copied() {
                        memory.set(mb.to_string());
                    }
                }),
        )
        .child(
            TextInput::new(memory)
                .width(Size::px(90.))
                .placeholder(oneclient_common::default_mem_max().to_string())
                .on_validate(validate_memory)
                .trailing(
                    label()
                        .text("MB")
                        .font_size(12.)
                        .color(colors::fg_secondary()),
                ),
        )
}
