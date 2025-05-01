#[cfg(feature = "specta")]
mod specta;

#[cfg(feature = "tauri")]
mod tauri;

mod pin;

/// This macro is used to derive the `specta::Type` trait for the annotated item.
/// It's mainly meant for entity-generation with sea-orm-cli (it doesn't like commas in attributes)
///
/// #### Expands to:
/// ```
/// #[cfg_attr(feature = "specta", derive(specta::Type))]
/// ```
///
/// ## `with_event` argument
/// Add `#[specta(with_event)]` to add the `tauri_specta::Event` derive.
///
/// #### Expands to:
/// ```
/// #[cfg_attr(feature = "specta", derive(specta::Type))]
/// #[cfg_attr(feature = "tauri", derive(tauri_specta::Event))]
/// ```
#[proc_macro_attribute]
#[cfg(feature = "specta")]
pub fn specta(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	specta::specta(attr, item)
}

/// This macro is used to derive the `tauri::command` as well as the `specta::specta` attribute for the annotated item.
/// #### Expands to:
/// ```
/// #[specta::specta]
/// #[tauri::command]
/// ```
#[proc_macro_attribute]
#[cfg(feature = "tauri")]
pub fn command(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	tauri::tauri_command(attr, item)
}

/// This macro Box::Pin a function
#[proc_macro_attribute]
pub fn pin(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	pin::pin(attr, item)
}
