
use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::ecs::entity::Entity;

use super::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};


pub type Sequence = SequencialAnd;
pub struct SequencialAnd {
    nodes: Vec<Arc<dyn Node>>,
}
impl SequencialAnd {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self { nodes })
    }
}
impl Node for SequencialAnd {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co| async move {
            for node in self.nodes.iter() {
                let mut gen = node.clone().run(world.clone(), entity);
                let node_result = complete_or_yield(&co, &mut gen).await;
                match node_result {
                    NodeResult::Success => {},
                    _ => { return node_result; },
                }
            }
            NodeResult::Success
        };
        Box::new(Gen::new(producer))
    }
}

pub type Selector = SequencialOr;
pub struct SequencialOr {
    nodes: Vec<Arc<dyn Node>>,
}
impl SequencialOr {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self { nodes })
    }
}
impl Node for SequencialOr {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co| async move {
            for node in self.nodes.iter() {
                let mut gen = node.clone().run(world.clone(), entity);
                let node_result = complete_or_yield(&co, &mut gen).await;
                match node_result {
                    NodeResult::Failure => {},
                    _ => { return node_result; },
                }
            }
            NodeResult::Failure
        };
        Box::new(Gen::new(producer))
    }
}
