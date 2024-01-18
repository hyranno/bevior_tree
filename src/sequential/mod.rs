use std::sync::Mutex;

use bevy::ecs::{system::{ReadOnlySystem, IntoSystem}, entity::Entity, world::World};

use crate::node::prelude::*;


pub mod variants;

pub mod prelude {
    pub use super::{
        Scorer, Picker, CondContinue, ResultConstructor,
        ScoredSequence,
        pair_node_scorer_fn,
        variants::prelude::*,
    };
}


pub trait Scorer: ReadOnlySystem<In=Entity, Out=f32> {}
impl<S> Scorer for S where S: ReadOnlySystem<In=Entity, Out=f32> {}

pub trait Picker: Fn(Vec<f32>) -> Vec<usize> + 'static + Send + Sync {}
impl<F> Picker for F where F: Fn(Vec<f32>) -> Vec<usize> + 'static + Send + Sync {}

pub trait CondContinue: Fn(NodeResult) -> bool + 'static + Send + Sync {}
impl<F> CondContinue for F where F: Fn(NodeResult) -> bool + 'static + Send + Sync {}

pub trait ResultConstructor: Fn(Vec<NodeResult>) -> NodeResult + 'static + Send + Sync {}
impl<F> ResultConstructor for F where F: Fn(Vec<NodeResult>) -> NodeResult + 'static + Send + Sync {}


#[with_state(ScoredSequenceState)]
pub struct ScoredSequence {
    nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>,
    picker: Box<dyn Picker>,
    cond_continue: Box<dyn CondContinue>,
    result_constructor: Box<dyn ResultConstructor>,
}
impl ScoredSequence {
    pub fn new(
        nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>,
        picker: impl Picker,
        cond_continue: impl CondContinue,
        result_constructor: impl ResultConstructor,
    ) -> Self {
        Self {
            nodes,
            picker: Box::new(picker),
            cond_continue: Box::new(cond_continue),
            result_constructor: Box::new(result_constructor),
        }
    }
}
impl Node for ScoredSequence {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        let scores = self.nodes.iter().map(
            |(_, scorer)| {
                let mut scorer = scorer.lock().expect("Failed to lock");
                scorer.initialize(world);
                scorer.run(entity, world)
            }
        ).collect();
        let indices = (*self.picker)(scores);
        let state = Box::new(ScoredSequenceState::new(indices));
        self.resume(world, entity, state)
    }

    fn resume(&self, world: &mut bevy::prelude::World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let state = Self::downcast(state).expect("Invalid state.");
        let Some(&index) = state.indices.iter().skip(state.count).next() else {
            let result = (*self.result_constructor)(state.results);
            return NodeStatus::Complete(result)
        };
        let (state, child_state) = state.extract_child_state();
        let node = &self.nodes[index].0;
        let status = match child_state {
            None => node.begin(world, entity),
            Some(s) => node.resume(world, entity, s),
        };
        match status {
            NodeStatus::Pending(child_state) => {
                NodeStatus::Pending(Box::new(state.update_pending(child_state)))
            },
            NodeStatus::Complete(result) => {
                let state = state.update_result(result);
                if (*self.cond_continue)(result) {
                    self.resume(world, entity, Box::new(state))
                } else {
                    let result = (*self.result_constructor)(state.results);
                    NodeStatus::Complete(result)
                }
            },
            NodeStatus::Beginning => panic!("Unexpected NodeStatus::Beginning."),
        }
    }

    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        let state = Self::downcast(state).expect("Invalid state.");
        let Some(&index) = state.indices.iter().skip(state.count).next() else {return};
        let (_, Some(child_state)) = state.extract_child_state() else {return};
        let node = &self.nodes[index].0;
        node.force_exit(world, entity, child_state)
    }
}


#[derive(NodeState)]
struct ScoredSequenceState {
    count: usize,
    indices: Vec<usize>,
    results: Vec<NodeResult>,
    child_state: Option<Box<dyn NodeState>>,
}
impl ScoredSequenceState {
    fn new(indices: Vec<usize>) -> Self {
        Self {
            count: 0,
            indices,
            results: vec![],
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
        Self {
            count: self.count + 1,
            indices: self.indices,
            results: self.results.into_iter().chain([result]).collect(),
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
            self.child_state
        )
    }
}


pub fn pair_node_scorer_fn<F, Marker>(node: impl Node, scorer: F) -> (Box<dyn Node>, Mutex<Box<dyn Scorer>>)
where
    F: IntoSystem<Entity, f32, Marker>,
    <F as IntoSystem<Entity, f32, Marker>>::System : Scorer,
{
    (Box::new(node), Mutex::new(Box::new(IntoSystem::into_system(scorer))))
}

