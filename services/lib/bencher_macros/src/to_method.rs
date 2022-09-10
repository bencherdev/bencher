use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::method_variant::MethodVariant;

pub fn impl_to_method(name_ident: &Ident, method_variants: &Vec<MethodVariant>) -> TokenStream {
    let mut token_stream = TokenStream::new();
    match_arms(method_variants, &mut token_stream);
    quote! {
        impl crate::ToMethod for #name_ident {
            fn to_method(&self) -> http::Method {
                match self {
                    #token_stream
                }
            }
        }
    }
}

fn match_arms(method_variants: &Vec<MethodVariant>, tokens: &mut TokenStream) {
    for method_variants in method_variants {
        match method_variants {
            MethodVariant::Method { ident, method } => tokens.extend(quote! {
                Self::#ident => http::Method::#method,
            }),
        }
    }
}
