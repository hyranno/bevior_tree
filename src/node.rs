
use std::any::Any;
use bevy::prelude::{World, Entity};

pub trait NodeState: Send + Sync{
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeResult {
    Success,
    Failure,
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

