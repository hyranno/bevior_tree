//! Sequencial composit nodes.

use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::ecs::entity::Entity;

use super::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};


pub struct SequenceWhile {
    nodes: Vec<Arc<dyn Node>>,
    cond: Box<dyn Fn(NodeResult) -> bool + 'static + Send + Sync>,
    complete_value: NodeResult,
}
impl SequenceWhile {
    pub fn new(
        nodes: Vec<Arc<dyn Node>>,
        cond: impl Fn(NodeResult)->bool + 'static + Send + Sync,
        complete_value: NodeResult
    ) -> Arc<Self> {
        Arc::new(Self { nodes, cond: Box::new(cond), complete_value })
    }
}
impl Node for SequenceWhile {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co| async move {
            for node in self.nodes.iter() {
                let mut gen = node.clone().run(world.clone(), entity);
                let node_result = complete_or_yield(&co, &mut gen).await;
                if !(self.cond)(node_result) {
                    return node_result;
                }
            }
            self.complete_value
        };
        Box::new(Gen::new(producer))
    }
}

pub type Sequence = SequencialAnd;
pub struct SequencialAnd {
    delegate: Arc<SequenceWhile>,
}
impl SequencialAnd {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: SequenceWhile::new(
            nodes, |res| res==NodeResult::Success, NodeResult::Success
        )})
    }
}
impl Node for SequencialAnd {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}

pub type Selector = SequencialOr;
pub struct SequencialOr {
    delegate: Arc<SequenceWhile>,
}
impl SequencialOr {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: SequenceWhile::new(
            nodes, |res| res==NodeResult::Failure, NodeResult::Failure
        )})
    }
}
impl Node for SequencialOr {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}

pub struct ForcedSequence {
    delegate: Arc<SequenceWhile>,
}
impl ForcedSequence {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: SequenceWhile::new(
            nodes, |_| true, NodeResult::Success
        )})
    }
}
impl Node for ForcedSequence {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
