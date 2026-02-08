//! Decorator nodes that convert the result of its child.

use bevy::ecs::{entity::Entity, world::World};

use crate::node::prelude::*;

pub mod variants;

pub mod prelude {
    pub use super::{ResultConverter, ConverterStrategy, variants::prelude::*};
}

#[cfg_attr(feature = "serde", typetag::serde(tag = "type"))]
pub trait ConverterStrategy: 'static + Send + Sync {
    fn convert(&self, result: NodeResult) -> NodeResult;
}

/// Node that converts the result of the child.
pub struct ResultConverter {
    child: Box<dyn Node>,
    converter: Box<dyn ConverterStrategy>,
}
impl ResultConverter {
    pub fn new(
        child: impl Node,
        converter: impl ConverterStrategy,
    ) -> Self {
        Self {
            child: Box::new(child),
            converter: Box::new(converter),
        }
    }
    fn convert(&self, status: NodeStatus) -> NodeStatus {
        match &status {
            &NodeStatus::Complete(result) => NodeStatus::Complete(self.converter.convert(result)),
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
