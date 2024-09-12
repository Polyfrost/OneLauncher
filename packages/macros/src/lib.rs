#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	unused_import_braces,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![deny(unused_must_use)]
#![allow(
	clippy::missing_errors_doc,
	clippy::future_not_send,
	clippy::module_name_repetitions
)]

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
