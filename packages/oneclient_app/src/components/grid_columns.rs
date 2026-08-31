use freya::prelude::*;

use super::Dropdown;

pub const GRID_COLUMNS_MIN: u8 = 1;
pub const GRID_COLUMNS_MAX: u8 = 5;

pub fn resolved_columns(columns: u8) -> usize {
    columns.clamp(GRID_COLUMNS_MIN, GRID_COLUMNS_MAX) as usize
}

pub fn grid_columns_picker(mut columns: State<u8>, height: f32) -> impl IntoElement {
    let options: Vec<String> = (GRID_COLUMNS_MIN..=GRID_COLUMNS_MAX)
        .map(|n| n.to_string())
        .collect();
    let selected = resolved_columns(*columns.read()).to_string();

    Dropdown::new(selected, options)
        .width(Size::px(60.))
        .height(Size::px(height))
        .on_select(move |idx: usize| columns.set(GRID_COLUMNS_MIN + idx as u8))
}

pub fn columns_for_items(columns: u8, items: usize) -> usize {
    resolved_columns(columns).min(items.max(1))
}

pub fn cell_width(total: f32, cols: usize, gap: f32) -> f32 {
    let cols = cols.max(1);
    ((total - (cols - 1) as f32 * gap) / cols as f32).max(0.)
}
