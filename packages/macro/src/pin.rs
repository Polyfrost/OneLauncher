use quote::quote;
use syn::{ItemFn, parse_macro_input};

pub fn pin(
	_attr: proc_macro::TokenStream,
	item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
	let input = parse_macro_input!(item as ItemFn);

	let attrs = &input.attrs;
	let vis = &input.vis;
	let sig = &input.sig;
	let body = &input.block;

	let result = quote! {
		#(#attrs)*
		#vis #sig {
			Box::pin(async move {
				#body
			}).await
		}
	};

	result.into()
}
