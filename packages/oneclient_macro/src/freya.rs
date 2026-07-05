use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields};

pub(crate) fn expand_icon_named(input: DeriveInput) -> Result<TokenStream, syn::Error> {
    let name = &input.ident;

    let data_enum = match input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "IconNamed can only be derived for enums",
            ));
        }
    };

    let mut match_arms = Vec::new();

    for variant in data_enum.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "IconNamed only supports unit enum variants",
            ));
        }

        let variant_ident = &variant.ident;
        let file_name = format!("icons/{}.svg", to_kebab_case(&variant_ident.to_string()));

        let doc_comment = format!("Icon preview:\n\n![{}](../{})", variant_ident, file_name);

        match_arms.push(quote! {
            #[doc = #doc_comment]
            Self::#variant_ident => #file_name,
        });
    }

    let expanded = quote! {
        impl #name {
            pub fn path(self) -> &'static str {
                match self {
                    #(#match_arms)*
                }
            }
        }
    };

    Ok(expanded)
}

fn to_kebab_case(s: &str) -> String {
    let mut kebab = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0
            && (ch.is_uppercase()
                || (ch.is_numeric() && !s.chars().nth(i - 1).unwrap().is_numeric()))
        {
            kebab.push('-');
        }
        kebab.push(ch.to_ascii_lowercase());
    }
    kebab
}
