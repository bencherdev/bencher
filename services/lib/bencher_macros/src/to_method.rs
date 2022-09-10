use proc_macro2::{Ident, TokenStream};
use quote::quote;

use crate::method_variant::MethodVariant;

pub fn impl_to_method(name_ident: &Ident, method_variants: &Vec<MethodVariant>) -> TokenStream {
    let mut variants_token_stream = TokenStream::new();
    match_arms(method_variants, &mut variants_token_stream);

    let mut token_stream = quote! {
        impl crate::ToMethod for #name_ident {
            fn to_method(&self) -> http::Method {
                match self {
                    #variants_token_stream
                }
            }
        }
    };

    token_stream.extend(&mut impl_display(name_ident, method_variants).into_iter());

    token_stream
}

fn match_arms(method_variants: &Vec<MethodVariant>, tokens: &mut TokenStream) {
    for method_variants in method_variants {
        match method_variants {
            MethodVariant::Method { ident, method } => tokens.extend(quote! {
                Self::#ident => http::Method::#method,
            }),
            MethodVariant::Parent {
                parent_ident,
                child_ident: _,
            } => tokens.extend(quote! {
                Self::#parent_ident(method) => method.to_method(),
            }),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Methods,
    Parents,
}

pub fn impl_display(name_ident: &Ident, method_variants: &Vec<MethodVariant>) -> TokenStream {
    let kind = method_variants
        .iter()
        .fold(Ok(None), |acc_kind, method_variant| {
            if let Ok(ok_kind) = acc_kind {
                let kind = match method_variant {
                    MethodVariant::Method { .. } => Kind::Methods,
                    MethodVariant::Parent { .. } => Kind::Parents,
                };
                if let Some(some_kind) = ok_kind {
                    if some_kind == kind {
                        acc_kind
                    } else {
                        Err("Kinds do not match.")
                    }
                } else {
                    Ok(Some(kind))
                }
            } else {
                acc_kind
            }
        })
        .unwrap()
        .unwrap();

    let kind = match kind {
        Kind::Methods => quote! {
            write!(f, "{}", self.to_method())

        },
        Kind::Parents => quote! {
            write!(f, "{} {}", self.to_method(), self.as_str())
        },
    };

    quote! {
        impl std::fmt::Display for #name_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                use crate::ToMethod;
                #kind
            }
        }
    }
}
