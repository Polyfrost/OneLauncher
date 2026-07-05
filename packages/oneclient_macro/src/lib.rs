#[cfg(feature = "freya")]
mod freya;

#[cfg(feature = "freya")]
#[proc_macro_derive(IconNamed)]
pub fn derive_icon_named(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);

    freya::expand_icon_named(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}