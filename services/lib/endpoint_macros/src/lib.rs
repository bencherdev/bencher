mod enum_keyword;
mod method_variant;
mod strip;
mod to_method;

use enum_keyword::Keyword;
use method_variant::MethodVariant;
use strip::strip_proc_attributes;

#[proc_macro_derive(IntoEndpoint)]
pub fn derive_into_endpoint(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let mut token_tree = input.into_iter();

    strip_proc_attributes(&mut token_tree);
    let _name = Keyword::name(&mut token_tree);
    let method_variants = MethodVariant::get_all(&mut token_tree);

    let token_stream = proc_macro2::TokenStream::new();

    token_stream.into()
}

#[proc_macro_derive(ToMethod)]
pub fn derive_to_method(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let mut token_tree = input.into_iter();

    strip_proc_attributes(&mut token_tree);
    let _name = Keyword::name(&mut token_tree);
    let _brace_group = Keyword::brace(&mut token_tree);

    let token_stream = proc_macro2::TokenStream::new();

    token_stream.into()
}
