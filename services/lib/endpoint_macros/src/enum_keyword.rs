use proc_macro2::{token_stream::IntoIter, Delimiter, Group, Ident, TokenTree};

const PUB: &str = "pub";
const ENUM: &str = "enum";

#[derive(Debug)]
pub enum Keyword {
    Pub,
    Enum,
    Name(Ident),
}

impl From<Ident> for Keyword {
    // pub enum MyEnum
    fn from(ident: Ident) -> Self {
        match ident.to_string().as_str() {
            // pub enum MyEnum
            // ^^^
            PUB => Self::Pub,
            // pub enum MyEnum
            //     ^^^^
            ENUM => Self::Enum,
            // pub enum MyEnum
            //          ^^^^^^
            _ => Self::Name(ident),
        }
    }
}

impl Keyword {
    // pub enum MyEnum
    pub fn name(token_tree: &mut IntoIter) -> Option<Ident> {
        let mut keyword_token_tree = token_tree.clone();
        let mut is_enum = false;
        while let Some(TokenTree::Ident(ident)) = keyword_token_tree.next() {
            match Keyword::from(ident) {
                // pub enum MyEnum
                // ^^^
                Keyword::Pub => {},
                // pub enum MyEnum
                //     ^^^^
                Keyword::Enum => is_enum = true,
                // pub enum MyEnum
                //          ^^^^^^
                Keyword::Name(name_ident) => {
                    if is_enum {
                        *token_tree = keyword_token_tree;
                        return Some(name_ident);
                    } else {
                        return None;
                    }
                },
            }
        }
        None
    }

    //  pub enum MyEnum { ... }
    pub fn brace(token_tree: &mut IntoIter) -> Option<Group> {
        let mut keyword_token_tree = token_tree.clone();
        //  pub enum MyEnum { ... }
        //                  ^     ^
        if let Some(TokenTree::Group(brace_group)) = keyword_token_tree.next() {
            if let Delimiter::Brace = brace_group.delimiter() {
                *token_tree = keyword_token_tree;
                return Some(brace_group);
            }
        }
        None
    }
}
