use proc_macro2::{token_stream::IntoIter, Ident, TokenStream, TokenTree};
use quote::quote;

use crate::enum_keyword::Keyword;

const GET_ONE: &str = "GetOne";
const GET_LS: &str = "GetLs";
const POST: &str = "Post";
const PUT: &str = "Put";
const DELETE: &str = "Delete";

#[derive(Debug)]
pub enum MethodVariant {
    Method { ident: Ident, method: TokenStream },
}

impl MethodVariant {
    //  ResourceVariant(MethodEnum),
    //  MethodVariant,
    fn new(token_tree: &mut IntoIter) -> Option<MethodVariant> {
        //  ResourceVariant(MethodEnum),
        //  ^^^^^^^^^^^^^^^
        //  MethodVariant,
        //  ^^^^^^^^^^^^^
        if let TokenTree::Ident(variant_ident) = token_tree.next()? {
            let method = match variant_ident.to_string().as_str() {
                GET_ONE | GET_LS => {
                    quote! {GET}
                },
                POST => {
                    quote! {POST}
                },
                PUT => {
                    quote! {PUT}
                },
                DELETE => {
                    quote! {DELETE}
                },
                _ => return None,
            };
            //  MethodVariant,
            //               ^
            if let TokenTree::Punct(comma_punct) = token_tree.next()? {
                if comma_punct.as_char() == ',' {
                    return Some(MethodVariant::Method {
                        ident: variant_ident,
                        method,
                    });
                }
            }
        }
        None
    }

    //  MethodVariantA,
    //  MethodVariantB,
    //  MethodVariantC,
    pub fn get_all(mut token_tree: &mut IntoIter) -> Option<Vec<MethodVariant>> {
        let mut method_variants = Vec::new();
        let brace_group = Keyword::brace(&mut token_tree)?;
        let mut brace_token_tree = brace_group.stream().into_iter();
        while let Some(method_variant) = Self::new(&mut brace_token_tree) {
            method_variants.push(method_variant);
        }
        Some(method_variants)
    }
}
