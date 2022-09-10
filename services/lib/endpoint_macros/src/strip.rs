use proc_macro2::{token_stream::IntoIter, Delimiter, TokenTree};

pub fn strip_proc_attributes(token_tree: &mut IntoIter) {
    while is_proc_attribute(token_tree) {}
}

fn is_proc_attribute(token_tree: &mut IntoIter) -> bool {
    let mut stripped_token_tree = token_tree.clone();
    if let Some(TokenTree::Punct(punct)) = stripped_token_tree.next() {
        if punct.as_char() == '#' {
            if let Some(TokenTree::Group(bracket_group)) = stripped_token_tree.next() {
                if let Delimiter::Bracket = bracket_group.delimiter() {
                    *token_tree = stripped_token_tree;
                    return true;
                }
            }
        }
    }
    false
}
