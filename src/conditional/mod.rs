//! Nodes that depends on the condition of the bevy world().

use std::sync::Mutex;

use bevy::ecs::{
    entity::Entity,
    system::{In, IntoSystem, System},
    world::World,
};

use crate::node::prelude::*;

pub mod variants;

pub mod prelude {
    pub use super::{
        CheckIf, ConditionalLoop, ElseFreeze, LoopCondChecker, LoopState, variants::prelude::*,
    };
}

pub trait LoopCondChecker: System<In = In<(Entity, LoopState)>, Out = bool> {}
impl<S> LoopCondChecker for S where S: System<In = In<(Entity, LoopState)>, Out = bool> {}

#[cfg_attr(feature = "serde", typetag::serde(tag = "type"))]
pub trait LoopCondCheckerBuilder: 'static + Send + Sync {
    fn build(&self) -> Box<dyn LoopCondChecker>;
}


#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LoopCountCondCheckerBuilder {
    max_count: usize,
}
#[cfg_attr(feature = "serde", typetag::serde)]
impl LoopCondCheckerBuilder for LoopCountCondCheckerBuilder {
    fn build(&self) -> Box<dyn LoopCondChecker> {
        let max_count = self.max_count;
        Box::new(IntoSystem::into_system(
            move |In((_, loop_state)): In<(Entity, LoopState)>| loop_state.count < max_count,
        ))
    }
}

/// Node for conditional loop.
#[with_state(ConditionalLoopState)]
pub struct ConditionalLoop {
    child: Box<dyn Node>,
    checker_builder: Box<dyn LoopCondCheckerBuilder>,
    // #[cfg_attr(feature = "serde", serde(skip))]
    checker_runtime: Mutex<Option<Box<dyn LoopCondChecker>>>,
}
impl ConditionalLoop {
    pub fn new(child: impl Node, checker_builder: impl LoopCondCheckerBuilder) -> Self {
        Self {
            child: Box::new(child),
            checker_builder: Box::new(checker_builder),
            checker_runtime: Mutex::new(None),
        }
    }
    pub fn check(&self, world: &mut World, entity: Entity, loop_state: LoopState) -> bool {
        let mut checker_lock = self.checker_runtime.lock().expect("Failed to lock.");
        if checker_lock.is_none() {
            let mut new_checker = self.checker_builder.build();
            new_checker.initialize(world);
            *checker_lock = Some(new_checker);
        }
        checker_lock.as_mut().expect("Checker not initialized.")
            .run((entity, loop_state), world)
            .expect("Failed to run checker system.")
    }
}
impl Node for ConditionalLoop {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        let state = ConditionalLoopState {
            loop_state: LoopState {
                count: 0,
                last_result: None,
            },
            child_status: NodeStatus::Beginning,
        };
        self.resume(world, entity, Box::new(state))
    }

    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let state = Self::downcast(state).expect("Invalid state type.");
        let state = match state.child_status {
            NodeStatus::Beginning => {
                if !self.check(world, entity, state.loop_state) {
                    return NodeStatus::Complete(
                        state.loop_state.last_result.unwrap_or(NodeResult::Failure),
                    );
                }
                ConditionalLoopState {
                    loop_state: state.loop_state,
                    child_status: self.child.begin(world, entity),
                }
            }
            NodeStatus::Pending(child_state) => ConditionalLoopState {
                loop_state: state.loop_state,
                child_status: self.child.resume(world, entity, child_state),
            },
            NodeStatus::Complete(result) => ConditionalLoopState {
                loop_state: state.loop_state.update(result),
                child_status: NodeStatus::Beginning,
            },
        };
        match &state.child_status {
            &NodeStatus::Beginning => self.resume(world, entity, Box::new(state)),
            &NodeStatus::Complete(_) => self.resume(world, entity, Box::new(state)),
            &NodeStatus::Pending(_) => NodeStatus::Pending(Box::new(state)),
        }
    }

    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        let state = Self::downcast(state).expect("Invalid state type.");
        match state.child_status {
            NodeStatus::Pending(child_state) => self.child.force_exit(world, entity, child_state),
            _ => {}
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(NodeState, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopState {
    count: usize,
    last_result: Option<NodeResult>,
}
impl LoopState {
    fn update(self, result: NodeResult) -> Self {
        Self {
            count: self.count + 1,
            last_result: Some(result),
        }
    }
}

/// State for [`ConditionalLoop`]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(NodeState)]
struct ConditionalLoopState {
    loop_state: LoopState,
    child_status: NodeStatus,
}

pub trait CondChecker: System<In = In<Entity>, Out = bool> {}
impl<S> CondChecker for S where S: System<In = In<Entity>, Out = bool> {}

#[cfg_attr(feature = "serde", typetag::serde(tag = "type"))]
pub trait CondCheckerBuilder: 'static + Send + Sync {
    fn build(&self) -> Box<dyn CondChecker>;
}

/// State for [`CheckIf`]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(NodeState, Debug)]
struct CheckIfState;

/// Node that check the condition, then return it as [`NodeResult`].
#[with_state(CheckIfState)]
pub struct CheckIf {
    checker_builder: Box<dyn CondCheckerBuilder>,
    // #[cfg_attr(feature = "serde", serde(skip))]
    checker_runtime: Mutex<Option<Box<dyn CondChecker>>>,
}
impl CheckIf {
    pub fn new(checker_builder: impl CondCheckerBuilder) -> Self {
        Self {
            checker_builder: Box::new(checker_builder),
            checker_runtime: Mutex::new(None),
        }
    }
    fn check(&self, world: &mut World, entity: Entity) -> bool {
        let mut checker_lock = self.checker_runtime.lock().expect("Failed to lock.");
        if checker_lock.is_none() {
            let mut new_checker = self.checker_builder.build();
            new_checker.initialize(world);
            *checker_lock = Some(new_checker);
        }
        checker_lock.as_mut().expect("Checker not initialized.")
            .run(entity, world)
            .expect("Failed to run checker system.")
    }
}
impl Node for CheckIf {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.resume(world, entity, Box::new(CheckIfState))
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let _state = Self::downcast(state).expect("Invalid state type.");
        NodeStatus::Complete(if self.check(world, entity) {
            NodeResult::Success
        } else {
            NodeResult::Failure
        })
    }
    fn force_exit(&self, _world: &mut World, _entity: Entity, _state: Box<dyn NodeState>) {
        // never
    }
}

/// Node that run the child while condition matched, else freeze.
/// Freezes transition of the child sub-tree, not running task.
#[with_state(ElseFreezeState)]
pub struct ElseFreeze {
    child: Box<dyn Node>,
    checker_builder: Box<dyn CondCheckerBuilder>,
    // #[cfg_attr(feature = "serde", serde(skip))]
    checker_runtime: Mutex<Option<Box<dyn CondChecker>>>,
}
impl ElseFreeze {
    pub fn new(child: impl Node, checker_builder: impl CondCheckerBuilder) -> Self {
        Self {
            child: Box::new(child),
            checker_builder: Box::new(checker_builder),
            checker_runtime: Mutex::new(None),
        }
    }
    fn check(&self, world: &mut World, entity: Entity) -> bool {
        let mut checker_lock = self.checker_runtime.lock().expect("Failed to lock.");
        if checker_lock.is_none() {
            let mut new_checker = self.checker_builder.build();
            new_checker.initialize(world);
            *checker_lock = Some(new_checker);
        }
        checker_lock.as_mut().expect("Checker not initialized.")
            .run(entity, world)
            .expect("Failed to run checker system.")
    }
}
impl Node for ElseFreeze {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.resume(
            world,
            entity,
            Box::new(ElseFreezeState {
                child_status: NodeStatus::Beginning,
            }),
        )
    }

    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let state = Self::downcast(state).expect("Invalid state.");
        if !self.check(world, entity) {
            return NodeStatus::Pending(Box::new(state));
        }
        let child_status = match state.child_status {
            NodeStatus::Beginning => self.child.begin(world, entity),
            NodeStatus::Pending(child_state) => self.child.resume(world, entity, child_state),
            NodeStatus::Complete(_) => {
                panic!("Invalid child status.")
            }
        };
        match &child_status {
            NodeStatus::Beginning => {
                panic!("Invalid child status.")
            }
            NodeStatus::Pending(_) => {
                NodeStatus::Pending(Box::new(ElseFreezeState { child_status }))
            }
            NodeStatus::Complete(_) => child_status,
        }
    }

    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        let state = Self::downcast(state).expect("Invalid state.");
        match state.child_status {
            NodeStatus::Pending(child_state) => self.child.force_exit(world, entity, child_state),
            _ => {}
        }
    }
}

/// State for [`ElseFreeze`]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(NodeState)]
struct ElseFreezeState {
    child_status: NodeStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tester_util::prelude::*;
    use bevy::state::app::StatesPlugin;

    #[derive(Component)]
    struct TestMarker;

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Default, States)]
    enum TestStates {
        #[default]
        MainState,
        FreezeState,
    }

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    struct TestMarkerExistsCondCheckerBuilder;
    #[cfg_attr(feature = "serde", typetag::serde)]
    impl CondCheckerBuilder for TestMarkerExistsCondCheckerBuilder {
        fn build(&self) -> Box<dyn CondChecker> {
            Box::new(IntoSystem::into_system(
                |In(entity): In<Entity>, world: &World| -> bool {
                    world.entity(entity).contains::<TestMarker>()
                },
            ))
        }
    }

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    struct TestStateMatcherCondCheckerBuilder {
        target_state: TestStates,
    }
    #[cfg_attr(feature = "serde", typetag::serde)]
    impl CondCheckerBuilder for TestStateMatcherCondCheckerBuilder {
        fn build(&self) -> Box<dyn CondChecker> {
            let target_state = self.target_state;
            Box::new(IntoSystem::into_system(
                move |In(_): In<Entity>, state: Res<State<TestStates>>| -> bool {
                    *state.get() == target_state
                },
            ))
        }
    }

    #[test]
    fn test_repeat_count() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask0::new(1, NodeResult::Success);
        let repeater =
            ConditionalLoop::new(task, LoopCountCondCheckerBuilder { max_count: 3 });
        let tree = BehaviorTree::from_node(
            repeater,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 0
        app.update(); // 1
        app.update(); // 2, repeater complete
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
            "ConditionalLoop should repeat the task. found: {:?}",
            found
        );
    }

    #[test]
    fn test_check_if_false() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = CheckIf::new(TestMarkerExistsCondCheckerBuilder);
        let tree = BehaviorTree::from_node(
            task,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update();
        let tree_status = app.world().get::<TreeStatus>(entity);
        assert!(
            match tree_status {
                Some(&TreeStatus(NodeStatus::Complete(NodeResult::Failure))) => true,
                _ => false,
            },
            "CheckIf should match the result."
        );
    }

    #[test]
    fn test_check_if_true() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = CheckIf::new(TestMarkerExistsCondCheckerBuilder);
        let tree = BehaviorTree::from_node(
            task,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let entity = app.world_mut().spawn((tree, TestMarker)).id();
        app.update();
        app.update();
        let tree_status = app.world().get::<TreeStatus>(entity);
        assert!(
            match tree_status {
                Some(&TreeStatus(NodeStatus::Complete(NodeResult::Success))) => true,
                _ => false,
            },
            "CheckIf should match the result."
        );
    }

    #[test]
    fn test_conditional_freeze() {
        let mut app = App::new();
        app.add_plugins((StatesPlugin, BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask0::new(2, NodeResult::Success);
        let root = ElseFreeze::new(task, TestStateMatcherCondCheckerBuilder {
            target_state: TestStates::MainState,
        });
        let tree = BehaviorTree::from_node(
            root,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.init_state::<TestStates>();
        app.update();
        app.update(); // 0
        app.world_mut()
            .get_resource_mut::<NextState<TestStates>>()
            .unwrap()
            .set(TestStates::FreezeState);
        app.update(); // 1
        app.update(); // 2
        app.world_mut()
            .get_resource_mut::<NextState<TestStates>>()
            .unwrap()
            .set(TestStates::MainState);
        app.update(); // 3, repeater complete
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 0,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 1,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 2,
                    frame: 3,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 3,
                    frame: 4,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "ElseFreeze should match the result. found: {:?}",
            found
        );
    }
}
