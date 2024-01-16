//! Composite nodes that run children parallelly.

use bevy::ecs::{entity::Entity, world::World};

use crate::node::prelude::*;


pub mod variants;

pub mod prelude {
    pub use super::{
        Parallel,
        variants::prelude::*,
    };
}


pub struct Parallel {
    children: Vec<Box<dyn Node>>,
    result_constructor: Box<dyn Fn(Vec<Option<NodeResult>>) -> Option<NodeResult> + 'static + Send + Sync>,
}
impl Parallel {
    pub fn new(
        children: Vec<Box<dyn Node>>,
        result_constructor: impl Fn(Vec<Option<NodeResult>>) -> Option<NodeResult> + 'static + Send + Sync,
    ) -> Self {
        Self {
            children,
            result_constructor: Box::new(result_constructor)
        }
    }
}
impl WithState<ParallelState> for Parallel {}
impl Node for Parallel {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        let state = ParallelState {
            children_status: self.children.iter().map(|_| NodeStatus::Beginning).collect(),
        };
        self.resume(world, entity, Box::new(state))
    }

    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let state = Self::downcast(state).expect("Invalid state.");
        if let Some(result) = (*self.result_constructor)(state.results()) {
            self.force_exit(world, entity, Box::new(state));
            return NodeStatus::Complete(result);
        }
        let children_status = self.children.iter().zip(state.children_status.into_iter()).map(
            |(child, child_status)|
            match child_status {
                NodeStatus::Beginning => child.begin(world, entity),
                NodeStatus::Pending(child_state) => child.resume(world, entity, child_state),
                NodeStatus::Complete(_) => child_status,
            }
        ).collect();
        let state = ParallelState {children_status};
        if let Some(result) = (*self.result_constructor)(state.results()) {
            self.force_exit(world, entity, Box::new(state));
            NodeStatus::Complete(result)
        } else {
            NodeStatus::Pending(Box::new(state))
        }
    }

    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        let state = Self::downcast(state).expect("Invalid state.");
        self.children.iter().zip(state.children_status.into_iter()).for_each(
            |(child, child_status)|
            match child_status {
                NodeStatus::Pending(child_state) => child.force_exit(world, entity, child_state),
                _ => {}
            }
        );
    }
}


struct ParallelState {
    children_status: Vec<NodeStatus>,
}
impl NodeState for ParallelState {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}
impl ParallelState {
    fn results(&self) -> Vec<Option<NodeResult>> {
        self.children_status.iter().map(
            |status|
            match status {
                &NodeStatus::Complete(result) => Some(result),
                _ => None
            }
        ).collect()
    }
}
