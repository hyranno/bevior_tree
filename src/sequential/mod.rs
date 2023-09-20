use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::ecs::{entity::Entity, system::{ReadOnlySystemParam, SystemParam, SystemState}};

use crate::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};

pub mod variants;

/// Node that runs children while `is_continue(result_of_child)` returns true.
/// Children are scored on run, then sorted or picked by the `picker`, according to their score.
/// Returns `complete_with` if `is_continue` returns true for the last picked child.
pub struct ScoredSequence {
    node_scorers: Mutex<Vec<Box<dyn NodeScorer>>>,
    picker: Box<dyn Fn(Vec<(f32, Arc<dyn Node>)>) -> Vec<(f32, Arc<dyn Node>)> + 'static + Send + Sync>,
    is_continue: Box<dyn Fn(NodeResult) -> bool + 'static + Send + Sync>,
    complete_with: NodeResult,
}
impl ScoredSequence {
    pub fn new(
        node_scorers: Vec<Box<dyn NodeScorer>>,
        picker: impl Fn(Vec<(f32, Arc<dyn Node>)>) -> Vec<(f32, Arc<dyn Node>)> + 'static + Send + Sync,
        is_continue: impl Fn(NodeResult) -> bool + 'static + Send + Sync,
        complete_with: NodeResult,
    ) -> Arc<Self> {
        Arc::new(Self {
            node_scorers: Mutex::new(node_scorers),
            picker: Box::new(picker),
            is_continue: Box::new(is_continue),
            complete_with,
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
            for (_, node) in nodes.iter() {
                let mut gen = node.clone().run(world.clone(), entity);
                let node_result = complete_or_yield(&co, &mut gen).await;
                if !(self.is_continue)(node_result) {
                    return node_result;
                }
            }
            self.complete_with
        };
        Box::new(Gen::new(producer))
    }
}


/// Returns a scored node.
/// Can be too versatile to implement by yourself, consider using `NodeScorerImpl` instead.
pub trait NodeScorer: 'static + Send + Sync {
    fn score(&mut self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity,) -> (f32, Arc<dyn Node>);
}


/// Pairs `Node` and `Scorer`, implementing `NodeScorer`.
pub struct NodeScorerImpl<S>
where
    S: Scorer + 'static,
{
    scorer: S,
    node: Arc<dyn Node>,
    system_state: Option<SystemState<S::Param<'static, 'static>>>,
}
impl<S> NodeScorerImpl<S>
where
    S: Scorer + 'static,
{
    pub fn new(scorer: S, node: Arc<dyn Node>) -> Self {
        Self { scorer, node, system_state: None }
    }
}
impl<S> NodeScorer for NodeScorerImpl<S>
where
    S: Scorer + 'static,
{
    fn score(&mut self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity,) -> (f32, Arc<dyn Node>) {
        let score = world.lock().unwrap().score_node(entity, &self.scorer, &mut self.system_state).unwrap();
        ( score, self.node.clone() )
    }
}


pub trait Scorer: Send + Sync {
    type Param<'w, 's>: ReadOnlySystemParam;
    fn score(
        &self,
        entity: Entity,
        param: <<Self as Scorer>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
    ) -> f32;
}

pub struct ConstantScorer {
    score: f32,
}
impl Scorer for ConstantScorer {
    type Param<'w, 's> = ();
    fn score(
        &self,
        _entity: Entity,
        _param: Self::Param<'_, '_>,
    ) -> f32 {
        self.score
    }
}
