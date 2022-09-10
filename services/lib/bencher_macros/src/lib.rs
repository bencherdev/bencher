mod enum_keyword;
mod method_variant;
mod strip;
mod to_method;

use enum_keyword::Keyword;
use method_variant::MethodVariant;
use strip::strip_proc_attributes;
use to_method::impl_to_method;

#[proc_macro_derive(ToMethod)]
pub fn derive_to_method(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let mut token_tree = input.into_iter();

    strip_proc_attributes(&mut token_tree);
    let name = Keyword::name(&mut token_tree).expect("Failed to find enum name.");
    let method_variants =
        MethodVariant::get_all(&mut token_tree).expect("Failed to parse method variants.");

    impl_to_method(&name, &method_variants).into()
}
