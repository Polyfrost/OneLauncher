use quote::quote;
use syn::{parse_macro_input, Item};

pub fn tauri_command(_attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let input = parse_macro_input!(item as Item);

	let expanded = quote! {
		#[specta::specta]
		#[tauri::command]
		#input
	};

	expanded.into()
}