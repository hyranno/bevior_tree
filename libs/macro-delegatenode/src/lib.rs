extern crate proc_macro;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, parse_macro_input};

/// Use attribute while standard way to delegate is not known.
#[proc_macro_attribute]
pub fn delegate_node(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_input = parse_macro_input!(attr as syn::Ident);
    let item_input = parse_macro_input!(item as ItemStruct);
    let delegate = attr_input;
    let node = item_input.ident.clone();

    let expand = quote! {
        #item_input
        impl bevior_tree::node::DelegateNode for #node {
            fn delegate_node(&self) -> &dyn bevior_tree::node::Node {
                &self.#delegate
            }
        }
    };
    TokenStream::from(expand)
}
