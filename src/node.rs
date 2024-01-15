
use std::{any::Any, ops::Not};
use bevy::prelude::{World, Entity};

pub mod prelude {
    pub use super::{Node, NodeStatus, NodeResult, NodeState,};
}


pub trait NodeState: Send + Sync{
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeResult {
    Success,
    Failure,
}
impl Not for NodeResult {
    type Output = NodeResult;
    fn not(self) -> Self::Output {
        match self {
            NodeResult::Success => NodeResult::Failure,
            NodeResult::Failure => NodeResult::Success,
        }
    }
}

pub enum NodeStatus {
    Beginning,
    Pending(Box<dyn NodeState>),
    Complete(NodeResult),
}

pub trait Node: 'static + Send + Sync {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus;
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus;
}

