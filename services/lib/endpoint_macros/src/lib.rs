#[proc_macro_derive(IntoEndpoint)]
pub fn derive_into_endpoint(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    input.into()
}
