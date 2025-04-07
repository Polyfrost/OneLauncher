use quote::quote;
use syn::{meta::ParseNestedMeta, parse_macro_input, Item, Result};

pub fn specta(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut attrs = SpectaAttributes::default();
	let parser = syn::meta::parser(|meta| attrs.parse(meta));
	parse_macro_input!(attr with parser);

	let input = parse_macro_input!(item as Item);

	let specta_event_type = if attrs.with_event {
		quote! {
			#[cfg_attr(feature = "tauri", derive(tauri_specta::Event))]
		}
	} else {
		quote! {}
	};

	let specta_type = quote! {
		#[cfg_attr(feature = "specta", derive(specta::Type))]
	};

	let expanded = quote! {
		#specta_event_type
		#specta_type
		#input
	};

	expanded.into()
}

#[derive(Default)]
struct SpectaAttributes {
    with_event: bool,
}

impl SpectaAttributes {
    fn parse(&mut self, meta: ParseNestedMeta) -> Result<()> {
		if meta.path.is_ident("with_event") {
			self.with_event = true;
		}
        Ok(())
    }
}