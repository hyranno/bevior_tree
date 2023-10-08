//! Composite nodes that run children sequentially.

use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::{ecs::entity::Entity, prelude::{ReadOnlySystem, IntoSystem}};

use crate::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};

pub mod variants;

/// Node that runs children while `is_continue(result_of_child)` returns true.
/// Children are scored on run, then sorted or picked by the `picker`, according to their score.
/// Returns `complete_with` if `is_continue` returns true for the last picked child.
pub struct ScoredSequence {
    node_scorers: Mutex<Vec<Box<dyn NodeScorer>>>,
    picker: Box<dyn Fn(Vec<(f32, Arc<dyn Node>)>) -> Vec<(f32, Arc<dyn Node>)> + 'static + Send + Sync>,
    is_continue: Box<dyn Fn(NodeResult) -> bool + 'static + Send + Sync>,
    complete_with: Box<dyn Fn(Vec<NodeResult>) -> NodeResult + 'static + Send + Sync>,
}
impl ScoredSequence {
    pub fn new(
        node_scorers: Vec<Box<dyn NodeScorer>>,
        picker: impl Fn(Vec<(f32, Arc<dyn Node>)>) -> Vec<(f32, Arc<dyn Node>)> + 'static + Send + Sync,
        is_continue: impl Fn(NodeResult) -> bool + 'static + Send + Sync,
        complete_with: impl Fn(Vec<NodeResult>) -> NodeResult + 'static + Send + Sync,
    ) -> Arc<Self> {
        Arc::new(Self {
            node_scorers: Mutex::new(node_scorers),
            picker: Box::new(picker),
            is_continue: Box::new(is_continue),
            complete_with: Box::new(complete_with),
        })
    }
}
impl Node for ScoredSequence {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let nodes = (self.picker)(
            self.node_scorers.lock().unwrap().iter_mut().map(|n|
                n.score(world.clone(), entity)
            ).collect()
        );
        let producer = |co| async move {
            let mut results = vec![];
            for (_, node) in nodes.iter() {
                let mut gen = node.clone().run(world.clone(), entity);
                let node_result = complete_or_yield(&co, &mut gen).await;
                if !(self.is_continue)(node_result) {
                    return node_result;
                }
                results.push(node_result);
            }
            (self.complete_with)(results)
        };
        Box::new(Gen::new(producer))
    }
}


/// Returns a scored node.
/// Can be too versatile to implement by yourself, consider using [`NodeScorerImpl`] instead.
pub trait NodeScorer: 'static + Send + Sync {
    fn score(&mut self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity,) -> (f32, Arc<dyn Node>);
}


/// Pairs [`Node`] and scorer, implementing [`NodeScorer`].
pub struct NodeScorerImpl {
    scorer: Box<dyn ReadOnlySystem<In=Entity, Out=f32>>,
    node: Arc<dyn Node>,
}
impl NodeScorerImpl {
    pub fn new<F, Marker>(scorer: F, node: Arc<dyn Node>) -> Self
    where
        F: IntoSystem<Entity, f32, Marker>,
        <F as IntoSystem<Entity, f32, Marker>>::System : ReadOnlySystem,
    {
        Self {
            scorer: Box::new(IntoSystem::into_system(scorer)),
            node,
        }
    }
}
impl NodeScorer for NodeScorerImpl {
    fn score(&mut self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity,) -> (f32, Arc<dyn Node>) {
        let score = world.lock().unwrap().score_node(entity, &mut self.scorer).unwrap();
        ( score, self.node.clone() )
    }
}
