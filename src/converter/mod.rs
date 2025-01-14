//! Decorator nodes that convert the result of its child.

use bevy::ecs::{entity::Entity, world::World};

use crate::node::prelude::*;

pub mod variants;

pub mod prelude {
    pub use super::{variants::prelude::*, ResultConverter};
}

/// Node that converts the result of the child.
pub struct ResultConverter {
    child: Box<dyn Node>,
    converter: Box<dyn Fn(NodeResult) -> NodeResult + 'static + Send + Sync>,
}
impl ResultConverter {
    pub fn new(
        child: impl Node,
        converter: impl Fn(NodeResult) -> NodeResult + 'static + Send + Sync,
    ) -> Self {
        Self {
            child: Box::new(child),
            converter: Box::new(converter),
        }
    }
    fn convert(&self, status: NodeStatus) -> NodeStatus {
        match &status {
            &NodeStatus::Complete(result) => NodeStatus::Complete((*self.converter)(result)),
            _ => status,
        }
    }
}
impl Node for ResultConverter {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.convert(self.child.begin(world, entity))
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.convert(self.child.resume(world, entity, state))
    }
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        self.child.force_exit(world, entity, state)
    }
}
