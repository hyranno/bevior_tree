use std::sync::{Arc, Mutex};

use bevy::ecs::entity::Entity;

use crate::{Node, NodeGen, nullable_access::NullableWorldAccess, NodeResult};
use super::ResultConverter;


/// Invert the result of the child.
pub struct Invert {
    delegate: Arc<ResultConverter>,
}
impl Node for Invert {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl Invert {
    pub fn new(child: Arc<dyn Node>) -> Arc<Self> {
        Arc::new(Self {
            delegate: ResultConverter::new(child, |res| !res)
        })
    }
}

/// Returns the specified result whatever the child returns.
pub struct ForceResult {
    delegate: Arc<ResultConverter>,
}
impl Node for ForceResult {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl ForceResult {
    pub fn new(child: Arc<dyn Node>, result: NodeResult) -> Arc<Self> {
        Arc::new(Self {
            delegate: ResultConverter::new(child, move |_| result.into())
        })
    }
}


#[cfg(test)]
mod tests {
    use crate::tester_util::*;

    #[test]
    fn test_invert() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let converter = Invert::new(task);
        let tree = BehaviorTree::new(converter);
        let entity = app.world.spawn(tree).id();
        app.update();
        app.update();
        let tree = app.world.get::<BehaviorTree>(entity).unwrap();
        assert!(
            tree.result.unwrap() == NodeResult::Failure,
            "Invert should match the result. found: {:?}", tree.result.unwrap() 
        );
    }

    #[test]
    fn test_force_result() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let converter = ForceResult::new(task, NodeResult::Failure);
        let tree = BehaviorTree::new(converter);
        let entity = app.world.spawn(tree).id();
        app.update();
        app.update();
        let tree = app.world.get::<BehaviorTree>(entity).unwrap();
        assert!(
            tree.result.unwrap() == NodeResult::Failure,
            "ForceResult should match the result. found: {:?}", tree.result.unwrap() 
        );
    }

}
