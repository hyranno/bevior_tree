extern crate proc_macro;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(NodeState)]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let target = input.ident;

    let expand = quote! {
        #[cfg_attr(feature = "serde", typetag::serde)]
        impl NodeState for #target {
            fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> { self }
        }
    };
    TokenStream::from(expand)
}
