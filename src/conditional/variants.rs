use bevy::ecs::{
    entity::Entity,
    system::{In, IntoSystem},
    world::World,
};

use super::{
    CondCheckerBuilder, ConditionalLoop, LoopCondChecker, LoopCondCheckerBuilder, LoopState,
};
use crate as bevior_tree;
use crate::node::prelude::*;

pub mod prelude {
    pub use super::{Conditional, InfiniteLoop};
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct OnceLoopCondCheckerBuilder {
    checker_builder: Box<dyn CondCheckerBuilder>,
}
impl OnceLoopCondCheckerBuilder {
    pub fn new(checker_builder: impl CondCheckerBuilder) -> Self {
        Self {
            checker_builder: Box::new(checker_builder),
        }
    }
}
#[cfg_attr(feature = "serde", typetag::serde)]
impl LoopCondCheckerBuilder for OnceLoopCondCheckerBuilder {
    fn build(&self) -> Box<LoopCondChecker> {
        let mut checker = self.checker_builder.build();
        Box::new(IntoSystem::into_system(
            move |In((entity, loop_state)): In<(Entity, LoopState)>, world: &mut World| {
                if loop_state.count < 1 && loop_state.last_result.is_none() {
                    checker.initialize(world);
                    checker.run(entity, world).expect("Failed to run checker")
                } else {
                    false
                }
            },
        ))
    }
}

/// Node that runs the child once if condition is matched.
#[delegate_node(delegate)]
pub struct Conditional {
    delegate: ConditionalLoop,
}
impl Conditional {
    pub fn new(child: impl Node, checker_builder: impl CondCheckerBuilder) -> Self {
        Self {
            delegate: ConditionalLoop::new(child, OnceLoopCondCheckerBuilder::new(checker_builder)),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct AlwaysLoopCondCheckerBuilder;
#[cfg_attr(feature = "serde", typetag::serde)]
impl LoopCondCheckerBuilder for AlwaysLoopCondCheckerBuilder {
    fn build(&self) -> Box<LoopCondChecker> {
        Box::new(IntoSystem::into_system(
            |In(_): In<(Entity, LoopState)>| -> bool { true },
        ))
    }
}

/// Node for infinite loop.
#[delegate_node(delegate)]
pub struct InfiniteLoop {
    delegate: ConditionalLoop,
}
impl InfiniteLoop {
    pub fn new(child: impl Node) -> Self {
        Self {
            delegate: ConditionalLoop::new(child, AlwaysLoopCondCheckerBuilder),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{CondChecker, CondCheckerBuilder};
    use crate::tester_util::prelude::*;

    #[derive(Component)]
    struct TestMarker;

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Debug)]
    struct TestMarkerExistsCondCheckerBuilder;
    #[cfg_attr(feature = "serde", typetag::serde)]
    impl CondCheckerBuilder for TestMarkerExistsCondCheckerBuilder {
        fn build(&self) -> Box<CondChecker> {
            Box::new(IntoSystem::into_system(
                |In(entity): In<Entity>, world: &World| -> bool {
                    world.entity(entity).contains::<TestMarker>()
                },
            ))
        }
    }

    #[test]
    fn test_conditional_false() {
        let mut app = App::new();
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let task = TesterTask0::new(1, NodeResult::Success);
        let conditional = Conditional::new(task, TestMarkerExistsCondCheckerBuilder);
        let tree = BehaviorTree::from_node(
            conditional,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // nop
        let expected = TestLog { log: vec![] };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Conditional should not do the task. Found {:?}",
            found
        );
    }

    #[test]
    fn test_conditional_true() {
        let mut app = App::new();
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let task = TesterTask0::new(1, NodeResult::Success);
        let conditional = Conditional::new(task, TestMarkerExistsCondCheckerBuilder);
        let tree = BehaviorTree::from_node(
            conditional,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn((tree, TestMarker)).id();
        app.update();
        app.update(); // 0
        app.update(); // nop
        let expected = TestLog {
            log: vec![TestLogEntry {
                task_id: 0,
                updated_count: 0,
                frame: 1,
            }],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Conditional should do the task. Found {:?}",
            found
        );
    }

    #[test]
    fn test_infinite_loop() {
        let mut app = App::new();
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let task = TesterTask0::new(1, NodeResult::Success);
        let repeater = InfiniteLoop::new(task);
        let tree = BehaviorTree::from_node(
            repeater,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 0
        app.update(); // 1
        app.update(); // 2
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 0,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 0,
                    frame: 3,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "InfiniteLoop should repeat the task. found: {:?}",
            found
        );
    }
}
