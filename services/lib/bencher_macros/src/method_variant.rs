use proc_macro2::{token_stream::IntoIter, Delimiter, Ident, TokenStream, TokenTree};
use quote::quote;

use crate::enum_keyword::Keyword;

const GET_ONE: &str = "GetOne";
const GET_LS: &str = "GetLs";
const POST: &str = "Post";
const PUT: &str = "Put";
const DELETE: &str = "Delete";

#[derive(Debug)]
pub enum MethodVariant {
    Method {
        ident: Ident,
        method: TokenStream,
    },
    Parent {
        parent_ident: Ident,
        child_ident: Ident,
    },
}

impl MethodVariant {
    //  ParentVariant(ChildEnum),
    //  MethodVariant,
    fn new(token_tree: &mut IntoIter) -> Option<MethodVariant> {
        //  ParentVariant(ChildEnum),
        //  ^^^^^^^^^^^^^^^
        //  MethodVariant,
        //  ^^^^^^^^^^^^^
        if let TokenTree::Ident(variant_ident) = token_tree.next()? {
            match token_tree.next()? {
                //  MethodVariant,
                //               ^
                TokenTree::Punct(punct) => {
                    if punct.as_char() == ',' {
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

                        return Some(MethodVariant::Method {
                            ident: variant_ident,
                            method,
                        });
                    }
                },
                //  ParentVariant(ChildEnum),
                //                 ^          ^
                TokenTree::Group(paren_group) => {
                    if let Delimiter::Parenthesis = paren_group.delimiter() {
                    } else {
                        return None;
                    }

                    //  ParentVariant(ChildEnum),
                    //                  ^^^^^^^^^^
                    let mut paren_token_tree = paren_group.stream().into_iter();
                    let child_ident = if let TokenTree::Ident(ident) = paren_token_tree.next()? {
                        ident
                    } else {
                        return None;
                    };

                    //  ParentVariant(ChildEnum),
                    //                            ^^
                    if let TokenTree::Punct(punct) = token_tree.next()? {
                        if paren_token_tree.next().is_none() && punct.as_char() == ',' {
                            return Some(MethodVariant::Parent {
                                parent_ident: variant_ident,
                                child_ident,
                            });
                        }
                    }
                },
                _ => {},
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
