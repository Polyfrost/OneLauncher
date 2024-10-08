use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Procedural macro that isolates a [`Box`] to ensure proper memory allocation in
/// debug environments.
#[proc_macro_attribute]
pub fn memory(_attr: TokenStream, item: TokenStream) -> TokenStream {
	let input = parse_macro_input!(item as ItemFn);

	let attrs = &input.attrs;
	let vis = &input.vis;
	let sig = &input.sig;
	let body = &input.block;

	#[cfg(debug_assertions)]
	let result = quote! {
		#(#attrs)*
		#vis #sig {
			Box::pin(async move {
				#body
			}).await
		}
	};
	#[cfg(not(debug_assertions))]
	let result = quote! {
		#(#attrs)*
		#vis #sig {
			#body
		}
	};

	TokenStream::from(result)
}
