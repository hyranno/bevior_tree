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
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[derive(Debug)]
        #item_input
        #[cfg_attr(feature = "serde", typetag::serde)]
        impl bevior_tree::node::Node for #node {
            fn begin(&self, world: &mut bevy::ecs::world::World, entity: bevy::ecs::entity::Entity) -> bevior_tree::node::NodeStatus {
                self.#delegate.begin(world, entity)
            }
            fn resume(&self, world: &mut bevy::ecs::world::World, entity: bevy::ecs::entity::Entity, state: Box<dyn bevior_tree::node::NodeState>) -> bevior_tree::node::NodeStatus {
                self.#delegate.resume(world, entity, state)
            }
            fn force_exit(&self, world: &mut bevy::ecs::world::World, entity: bevy::ecs::entity::Entity, state: Box<dyn bevior_tree::node::NodeState>) {
                self.#delegate.force_exit(world, entity, state)
            }
        }
    };
    TokenStream::from(expand)
}
