use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields};

pub fn error_attr(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

	let Data::Enum(data_enum) = &input.data else {
		return syn::Error::new_spanned(&input.ident, "Only enums are supported")
			.to_compile_error()
			.into();
	};

	let ident = &input.ident;
	let mut serialize_arms = Vec::new();

	for variant in &data_enum.variants {
		let v_ident = &variant.ident;
		match &variant.fields {
			Fields::Unit => {
				serialize_arms.push(quote! {
					Self::#v_ident => {
						let mut s = serializer.serialize_struct(stringify!(#ident), 2)?;
						s.serialize_field("type", stringify!(#v_ident))?;
						s.serialize_field("data", &self.to_string())?;
						s.end()
					}
				});
			}
			Fields::Unnamed(fields_unnamed) => {
				if fields_unnamed.unnamed.len() == 1 {
					let field = &fields_unnamed.unnamed[0];
					let mut has_from = false;
					let mut has_skip = false;

					for attr in &field.attrs {
						if attr.path().is_ident("from") {
							has_from = true;
						} else if attr.path().is_ident("skip") {
							has_skip = true;
						}
					}

					if has_from && !has_skip {
						serialize_arms.push(quote! {
							Self::#v_ident(inner) => {
								let mut s = serializer.serialize_struct(stringify!(#ident), 2)?;
								s.serialize_field("type", stringify!(#v_ident))?;
								s.serialize_field("data", inner)?;
								s.end()
							}
						});
					} else {
						serialize_arms.push(quote! {
							Self::#v_ident(_) => {
								let mut s = serializer.serialize_struct(stringify!(#ident), 2)?;
								s.serialize_field("type", stringify!(#v_ident))?;
								s.serialize_field("data", &self.to_string())?;
								s.end()
							}
						});
					}
				} else {
					serialize_arms.push(quote! {
						Self::#v_ident(..) => {
							let mut s = serializer.serialize_struct(stringify!(#ident), 2)?;
							s.serialize_field("type", stringify!(#v_ident))?;
							s.serialize_field("data", &self.to_string())?;
							s.end()
						}
					});
				}
			}
			Fields::Named(_) => {
				serialize_arms.push(quote! {
					Self::#v_ident { .. } => {
						let mut s = serializer.serialize_struct(stringify!(#ident), 2)?;
						s.serialize_field("type", stringify!(#v_ident))?;
						s.serialize_field("data", &self.to_string())?;
						s.end()
					}
				});
			}
		}
	}

	let serialize_impl = quote! {
		impl serde::Serialize for #ident {
			fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
			where
				S: serde::ser::Serializer,
			{
				use serde::ser::SerializeStruct;
				match self {
					#(#serialize_arms),*
				}
			}
		}
	};

    let output = quote! {
        #[cfg_attr(feature = "specta", derive(onelauncher_macro::SerializedError))]
        #input

		#serialize_impl
    };

    output.into()
}

pub fn error_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let enum_name = &input.ident;
    let specta_name = format_ident!("{}SpectaType", enum_name);
    let mut variants = vec![];

    let Data::Enum(data_enum) = &input.data else {
        return syn::Error::new_spanned(enum_name, "Only enums are supported")
            .to_compile_error()
            .into();
    };

    for variant in &data_enum.variants {
        let name = &variant.ident;
        let ty = match &variant.fields {
            Fields::Unit => quote! { String },

            Fields::Unnamed(fields) => {
                // Handle newtype-style or tuple-like enums
                if fields.unnamed.len() == 1 {
                    let field = &fields.unnamed[0];
                    let field_ty = &field.ty;

                    let has_from = field
                        .attrs
                        .iter()
                        .any(|a| a.path().is_ident("from"));

                    let has_skip = field
                        .attrs
                        .iter()
                        .any(|a| a.path().is_ident("skip"));

                    if has_from && !has_skip {
                        quote! {
							#field_ty
						}
                    } else {
                        quote! { String }
                    }
                } else {
                    quote! { String }
                }
            }

            Fields::Named(_) => {
                quote! { String } // Simplification: treat all struct-like variants as String
            }
        };

        variants.push(quote! {
            #name(#ty)
        });
    }

	let enum_name_str = enum_name.to_string();
	let output = quote! {
        #[derive(Debug, specta::Type, serde::Serialize)]
        #[specta(remote = #enum_name)]
		#[specta(tag = "type", content = "data")]
		#[specta(rename = #enum_name_str)]
        pub enum #specta_name {
            #(#variants,)*
        }
    };

    output.into()
}