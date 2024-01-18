extern crate proc_macro;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemStruct, TypePath};


/// Use attribute while `derive(WithState<State>)` is unavailable.
#[proc_macro_attribute]
pub fn with_state(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_input = parse_macro_input!(attr as TypePath);
    let item_input = parse_macro_input!(item as ItemStruct);
    let state = attr_input;
    let node = item_input.ident.clone();

    let expand = quote! {
        #item_input
        impl WithState<#state> for #node {}
    };
    TokenStream::from(expand)
}
