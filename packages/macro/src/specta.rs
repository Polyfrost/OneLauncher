use proc_macro2::TokenStream;
use quote::quote;
use syn::{meta::ParseNestedMeta, parse_macro_input, Item};

pub fn specta(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	let mut attrs = SpectaAttributes::default();
	let parser = syn::meta::parser(|meta| attrs.parse(meta));
	parse_macro_input!(attr with parser);

	let input = parse_macro_input!(item as Item);

    let specta_rename = if let Some(rename) = attrs.rename {
        quote! {
            #[cfg_attr(feature = "specta", specta(rename = #rename))]
        }
    } else {
        TokenStream::new()
    };

	let expanded = quote! {
		#[cfg_attr(feature = "specta", derive(specta::Type))]
		#specta_rename
		#input
	};

	expanded.into()
}

#[derive(Default)]
struct SpectaAttributes {
	rename: Option<String>,
}

impl SpectaAttributes {
    fn parse(&mut self, meta: ParseNestedMeta) -> syn::Result<()> {
		if meta.path.is_ident("rename") {
			self.rename = Some(meta.value()?.to_string());
		}
        Ok(())
    }
}