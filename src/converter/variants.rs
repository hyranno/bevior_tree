
use bevy::ecs::{entity::Entity, world::World};

use crate::node::prelude::*;
use super::ResultConverter;


pub mod prelude {
    pub use super::{
        Invert, ForceResult,
    };
}


/// Invert the result of the child.
pub struct Invert {
    delegate: ResultConverter,
}
impl Invert {
    pub fn new(child: impl Node) -> Self {
        Self {
            delegate: ResultConverter::new(child, |res| !res)
        }
    }
}
impl Node for Invert {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
}

/// Returns the specified result whatever the child returns.
pub struct ForceResult {
    delegate: ResultConverter,
}
impl ForceResult {
    pub fn new(child: impl Node, result: NodeResult) -> Self {
        Self {
            delegate: ResultConverter::new(child, move |_| result)
        }
    }
}
impl Node for ForceResult {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
}



#[cfg(test)]
mod tests {
    use crate::tester_util::prelude::*;

    #[test]
    fn test_invert() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let converter = Invert::new(task);
        let entity = app.world.spawn(BehaviorTreeBundle::from_root(converter)).id();
        app.update();
        app.update();
        let status = app.world.get::<TreeStatus>(entity);
        assert!(
            if let Some(TreeStatus(NodeStatus::Complete(result))) = status {
                result == &NodeResult::Failure
            } else {false},
            "Invert should match the result."
        );
    }

    #[test]
    fn test_force_result() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let converter = ForceResult::new(task, NodeResult::Failure);
        let entity = app.world.spawn(BehaviorTreeBundle::from_root(converter)).id();
        app.update();
        app.update();
        let status = app.world.get::<TreeStatus>(entity);
        assert!(
            if let Some(TreeStatus(NodeStatus::Complete(result))) = status {
                result == &NodeResult::Failure
            } else {false},
            "ForceResult should match the result."
        );
    }

}

