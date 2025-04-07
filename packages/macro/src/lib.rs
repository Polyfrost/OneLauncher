mod specta;
mod pin;

/// This macro is used to derive the `specta::Type` trait for the annotated item.
/// It's mainly meant for entity-generation with sea-orm-cli (it doesn't like commas in attributes)
/// <br>
/// Expands to:
/// ```
/// #[cfg_attr(feature = "specta", derive(specta::Type))]
/// ```
#[proc_macro_attribute]
pub fn specta(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	specta::specta(attr, item)
}

/// This macro Box::Pin a function
#[proc_macro_attribute]
pub fn pin(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	pin::pin(attr, item)
}
