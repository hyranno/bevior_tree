//! Composite nodes that run children in sequence.

use std::{fmt::Debug, sync::Mutex};

use bevy::ecs::{
    entity::Entity,
    system::{In, System},
    world::World,
};

use crate::node::prelude::*;

pub mod variants;

pub mod prelude {
    pub use super::{
        Picker, PickerBuilder, ResultStrategy, ScoredSequence, Scorer, ScorerBuilder,
        variants::prelude::*,
    };
}

pub type Scorer = dyn System<In = In<Entity>, Out = f32>;

#[cfg_attr(feature = "serde", typetag::serde(tag = "type"))]
pub trait ScorerBuilder: Debug + Send + Sync {
    fn build(&self) -> Box<Scorer>;
}

pub type Picker = dyn System<In = In<(Vec<f32>, Entity)>, Out = Vec<usize>>;

#[cfg_attr(feature = "serde", typetag::serde(tag = "type"))]
pub trait PickerBuilder: 'static + Debug + Send + Sync {
    fn build(&self) -> Box<Picker>;
}

#[cfg_attr(feature = "serde", typetag::serde(tag = "type"))]
pub trait ResultStrategy: 'static + Debug + Send + Sync {
    fn construct(&self, results: Vec<Option<NodeResult>>) -> Option<NodeResult>;
}

/// Composite nodes that run children in sequence.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
#[with_state(ScoredSequenceState)]
pub struct ScoredSequence {
    children: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>,
    picker: Box<dyn PickerBuilder>,
    result_strategy: Box<dyn ResultStrategy>,
    #[cfg_attr(feature = "serde", serde(skip))]
    scorers_runtime: Mutex<Vec<Box<Scorer>>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    picker_runtime: Mutex<Option<Box<Picker>>>,
}
impl ScoredSequence {
    pub fn new(
        children: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>,
        picker: impl PickerBuilder,
        result_strategy: impl ResultStrategy,
    ) -> Self {
        Self {
            children,
            picker: Box::new(picker),
            result_strategy: Box::new(result_strategy),
            scorers_runtime: Mutex::new(Vec::new()),
            picker_runtime: Mutex::new(None),
        }
    }
    fn init(&self, world: &mut World) {
        let mut scorers_runtime = self.scorers_runtime.lock().expect("Failed to lock");
        if scorers_runtime.is_empty() {
            *scorers_runtime = self
                .children
                .iter()
                .map(|(_, builder)| {
                    let mut scorer = builder.build();
                    scorer.initialize(world);
                    scorer
                })
                .collect();
        }
        let mut picker_runtime = self.picker_runtime.lock().expect("Failed to lock");
        if picker_runtime.is_none() {
            let mut picker = self.picker.build();
            picker.initialize(world);
            *picker_runtime = Some(picker);
        }
    }
}
#[cfg_attr(feature = "serde", typetag::serde)]
impl Node for ScoredSequence {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.init(world);
        let scores = self
            .scorers_runtime
            .lock()
            .expect("Failed to lock")
            .iter_mut()
            .map(|scorer| scorer.run(entity, world).expect("Scorer failed"))
            .collect();
        let mut picker_lock = self.picker_runtime.lock().expect("Failed to lock");
        let indices = picker_lock
            .as_mut()
            .expect("Picker not initialized")
            .run((scores, entity), world)
            .expect("Picker failed");
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
            let Some(result) = self.result_strategy.construct(state.results) else {
                panic!("Result constructor returned None on the end.");
            };
            return NodeStatus::Complete(result);
        };
        let (state, child_state) = state.extract_child_state();
        let node = &self.children[index].0;
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
                let result = self.result_strategy.construct(state.results.clone());
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
        let node = &self.children[index].0;
        node.force_exit(world, entity, child_state)
    }
}

/// State for [`ScoredSequence`]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(NodeState, Debug)]
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
