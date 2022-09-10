mod enum_keyword;
mod strip;

use enum_keyword::Keyword;
use strip::strip_proc_attributes;

#[proc_macro_derive(IntoEndpoint)]
pub fn derive_into_endpoint(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let mut token_tree = input.into_iter();

    strip_proc_attributes(&mut token_tree);
    let _name = Keyword::name(&mut token_tree);
    let _brace_group = Keyword::brace(&mut token_tree);

    let token_stream = proc_macro2::TokenStream::new();

    token_stream.into()
}
