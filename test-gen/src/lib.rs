use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemFn};

mod gen;

#[proc_macro_attribute]
pub fn wast(attr: TokenStream, item: TokenStream) -> TokenStream {
    let f = parse_macro_input!(item as ItemFn);

    TokenStream::from(gen::impl_tests(proc_macro2::TokenStream::from(attr), f))
}