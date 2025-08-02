mod error;
mod pin;
mod specta;

/// This macro is used to derive the `specta::Type` trait for the annotated item.
/// It's mainly meant for entity-generation with sea-orm-cli (it doesn't like commas in attributes)
///
/// #### Expands to:
/// ```
/// #[cfg_attr(feature = "specta", derive(specta::Type))]
/// ```
///
/// ## `rename` attribute
/// Add `rename = ''` attribute to rename the type.
///
/// #### Expands to:
/// ```
/// #[cfg_attr(feature = "specta", derive(specta::Type))]
/// #[cfg_attr(feature = "specta", specta(rename = #rename))]
/// ```
#[proc_macro_attribute]
pub fn specta(
	attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	specta::specta(attr, item)
}

/// This macro is used to implement the `serde::Serialize` trait for the annotated item.
/// It only works for enums and tries to serialize them so that taurpc can use them properly.
#[proc_macro_derive(SerializedError, attributes(from, skip))]
pub fn serialized_error_derive(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	error::error_derive(item)
}

#[proc_macro_attribute]
pub fn error(
	attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	error::error_attr(attr, item)
}

/// This macro `Box::Pin` a function
#[proc_macro_attribute]
pub fn pin(
	attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	pin::pin(attr, item)
}
