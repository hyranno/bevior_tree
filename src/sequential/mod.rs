//! Composite nodes that run children in sequence.

use std::sync::Mutex;

use bevy::ecs::{
    entity::Entity,
    system::{In, IntoSystem, ReadOnlySystem, System},
    world::World,
};

use crate::node::prelude::*;

pub mod variants;

pub mod prelude {
    pub use super::{
        Picker, ResultConstructor, ScoredSequence, Scorer, pair_node_scorer_fn,
        variants::prelude::*,
    };
}

pub trait Scorer: ReadOnlySystem<In = In<Entity>, Out = f32> {}
impl<S> Scorer for S where S: ReadOnlySystem<In = In<Entity>, Out = f32> {}

pub trait Picker: System<In = In<(Vec<f32>, Entity)>, Out = Vec<usize>> {}
impl<S> Picker for S where S: System<In = In<(Vec<f32>, Entity)>, Out = Vec<usize>> {}

pub trait ResultConstructor:
    Fn(Vec<Option<NodeResult>>) -> Option<NodeResult> + 'static + Send + Sync
{
}
impl<F> ResultConstructor for F where
    F: Fn(Vec<Option<NodeResult>>) -> Option<NodeResult> + 'static + Send + Sync
{
}

/// Composite nodes that run children in sequence.
#[with_state(ScoredSequenceState)]
pub struct ScoredSequence {
    nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>,
    picker: Mutex<Box<dyn Picker>>,
    result_constructor: Box<dyn ResultConstructor>,
}
impl ScoredSequence {
    pub fn new<P, Marker>(
        nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>,
        picker: P,
        result_constructor: impl ResultConstructor,
    ) -> Self
    where
        P: IntoSystem<In<(Vec<f32>, Entity)>, Vec<usize>, Marker>,
        <P as IntoSystem<In<(Vec<f32>, Entity)>, Vec<usize>, Marker>>::System: Picker,
    {
        Self {
            nodes,
            picker: Mutex::new(Box::new(IntoSystem::into_system(picker))),
            result_constructor: Box::new(result_constructor),
        }
    }
}
impl Node for ScoredSequence {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        let scores = self
            .nodes
            .iter()
            .map(|(_, scorer)| {
                let mut scorer = scorer.lock().expect("Failed to lock");
                scorer.initialize(world);
                scorer.run(entity, world).expect("Scorer failed")
            })
            .collect();
        let mut picker = self.picker.lock().expect("Failed to lock");
        picker.initialize(world);
        let indices = picker.run((scores, entity), world).expect("Picker failed");
        let state = Box::new(ScoredSequenceState::new(indices));
        self.resume(world, entity, state)
    }

    fn resume(
        &self,
        world: &mut bevy::prelude::World,
        entity: Entity,
        state: Box<dyn NodeState>,
    ) -> NodeStatus {
        let state = Self::downcast(state).expect("Invalid state.");
        let Some(&index) = state.indices.iter().skip(state.count).next() else {
            // All the nodes are completed.
            let Some(result) = (*self.result_constructor)(state.results) else {
                panic!("Result constructor returned None on the end.");
            };
            return NodeStatus::Complete(result);
        };
        let (state, child_state) = state.extract_child_state();
        let node = &self.nodes[index].0;
        let child_status = match child_state {
            None => node.begin(world, entity),
            Some(s) => node.resume(world, entity, s),
        };
        match child_status {
            NodeStatus::Pending(child_state) => {
                NodeStatus::Pending(Box::new(state.update_pending(child_state)))
            }
            NodeStatus::Complete(child_result) => {
                let state = state.update_result(child_result);
                let result = (*self.result_constructor)(state.results.clone());
                match result {
                    Some(result) => NodeStatus::Complete(result),
                    None => self.resume(world, entity, Box::new(state)),
                }
            }
            NodeStatus::Beginning => panic!("Unexpected NodeStatus::Beginning."),
        }
    }

    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        let state = Self::downcast(state).expect("Invalid state.");
        let Some(&index) = state.indices.iter().skip(state.count).next() else {
            return;
        };
        let (_, Some(child_state)) = state.extract_child_state() else {
            return;
        };
        let node = &self.nodes[index].0;
        node.force_exit(world, entity, child_state)
    }
}

/// State for [`ScoredSequence`]
#[derive(NodeState)]
struct ScoredSequenceState {
    count: usize,
    indices: Vec<usize>,
    results: Vec<Option<NodeResult>>,
    child_state: Option<Box<dyn NodeState>>,
}
impl ScoredSequenceState {
    fn new(indices: Vec<usize>) -> Self {
        let results = indices.iter().map(|_| None).collect();
        Self {
            count: 0,
            indices,
            results,
            child_state: None,
        }
    }
    fn update_pending(self, child_state: Box<dyn NodeState>) -> Self {
        Self {
            count: self.count,
            indices: self.indices,
            results: self.results,
            child_state: Some(child_state),
        }
    }
    fn update_result(self, result: NodeResult) -> Self {
        let results = self
            .results
            .into_iter()
            .enumerate()
            .map(|(index, v)| if index == self.count { Some(result) } else { v })
            .collect();
        Self {
            count: self.count + 1,
            indices: self.indices,
            results,
            child_state: None,
        }
    }
    fn extract_child_state(self) -> (Self, Option<Box<dyn NodeState>>) {
        (
            Self {
                count: self.count,
                indices: self.indices,
                results: self.results,
                child_state: None,
            },
            self.child_state,
        )
    }
}

pub fn pair_node_scorer_fn<F, Marker>(
    node: impl Node,
    scorer: F,
) -> (Box<dyn Node>, Mutex<Box<dyn Scorer>>)
where
    F: IntoSystem<In<Entity>, f32, Marker>,
    <F as IntoSystem<In<Entity>, f32, Marker>>::System: Scorer,
{
    (
        Box::new(node),
        Mutex::new(Box::new(IntoSystem::into_system(scorer))),
    )
}
