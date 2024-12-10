//! Nodes that depends on the condition of the bevy world().

use std::sync::Mutex;

use bevy::ecs::{entity::Entity, system::{IntoSystem, ReadOnlySystem, In}, world::World};

use crate::node::prelude::*;


pub mod variants;

pub mod prelude {
    pub use super::{
        LoopCondChecker,
        LoopState,
        ConditionalLoop, CheckIf, ElseFreeze,
        variants::prelude::*,
    };
}


pub trait LoopCondChecker: ReadOnlySystem<In=In<(Entity, LoopState)>, Out=bool> {}
impl<S> LoopCondChecker for S where S: ReadOnlySystem<In=In<(Entity, LoopState)>, Out=bool> {}


/// Node for conditional loop.
#[with_state(ConditionalLoopState)]
pub struct ConditionalLoop {
    child: Box<dyn Node>,
    checker: Mutex<Box<dyn LoopCondChecker>>,
}
impl ConditionalLoop {
    pub fn new<S, Marker>(node: impl Node, checker: S) -> Self
    where
        S: IntoSystem<In<(Entity, LoopState)>, bool, Marker>,
        <S as IntoSystem<In<(Entity, LoopState)>, bool, Marker>>::System : LoopCondChecker,
    {
        Self {
            child: Box::new(node),
            checker: Mutex::new(Box::new(IntoSystem::into_system(checker)))
        }
    }

    pub fn check(&self, world: &mut World, entity: Entity, loop_state: LoopState) -> bool {
        let mut checker = self.checker.lock().expect("Failed to lock.");
        checker.initialize(world);
        checker.run((entity, loop_state), world)
    }
}
impl Node for ConditionalLoop {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        let state = ConditionalLoopState {
            loop_state: LoopState { count: 0, last_result: None },
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
                        state.loop_state.last_result.unwrap_or(NodeResult::Failure)
                    )
                }
                ConditionalLoopState {
                    loop_state: state.loop_state,
                    child_status: self.child.begin(world, entity),
                }
            },
            NodeStatus::Pending(child_state) => {
                ConditionalLoopState {
                    loop_state: state.loop_state,
                    child_status: self.child.resume(world, entity, child_state),
                }
            },
            NodeStatus::Complete(result) => {
                ConditionalLoopState {
                    loop_state: state.loop_state.update(result),
                    child_status: NodeStatus::Beginning,
                }
            },
        };
        match &state.child_status {
            &NodeStatus::Beginning => {
                self.resume(world, entity, Box::new(state))
            },
            &NodeStatus::Complete(_) => {
                self.resume(world, entity, Box::new(state))
            },
            &NodeStatus::Pending(_) => {
                NodeStatus::Pending(Box::new(state))
            },
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
#[derive(NodeState)]
struct ConditionalLoopState {
    loop_state: LoopState,
    child_status: NodeStatus,
}


/// State for [`CheckIf`]
#[derive(NodeState, Debug)]
struct CheckIfState;

/// Node that check the condition, then return it as [`NodeResult`].
#[with_state(CheckIfState)]
pub struct CheckIf {
    checker: Mutex<Box<dyn ReadOnlySystem<In=In<Entity>, Out=bool>>>,
}
impl CheckIf {
    pub fn new<F, Marker>(checker: F) -> Self
    where
        F: IntoSystem<In<Entity>, bool, Marker>,
        <F as IntoSystem<In<Entity>, bool, Marker>>::System : ReadOnlySystem,
    {
        Self {
            checker: Mutex::new(Box::new(IntoSystem::into_system(checker))),
        }
    }

    fn check(&self, world: &mut World, entity: Entity) -> bool {
        let mut checker = self.checker.lock().expect("Failed to lock.");
        checker.initialize(world);
        checker.run(entity, world)
    }
}
impl Node for CheckIf {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.resume(world, entity, Box::new(CheckIfState))
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let _state = Self::downcast(state).expect("Invalid state type.");
        NodeStatus::Complete(
            if self.check(world, entity) {NodeResult::Success} else {NodeResult::Failure}
        )
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
    checker: Mutex<Box<dyn ReadOnlySystem<In=In<Entity>, Out=bool>>>,
}
impl ElseFreeze {
    pub fn new<F, Marker>(child: impl Node, checker: F) -> Self
    where
        F: IntoSystem<In<Entity>, bool, Marker>,
        <F as IntoSystem<In<Entity>, bool, Marker>>::System : ReadOnlySystem,
    {
        Self {
            child: Box::new(child),
            checker: Mutex::new(Box::new(IntoSystem::into_system(checker))),
        }
    }

    fn check(&self, world: &mut World, entity: Entity) -> bool {
        let mut checker = self.checker.lock().expect("Failed to lock.");
        checker.initialize(world);
        checker.run(entity, world)
    }
}
impl Node for ElseFreeze {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.resume(world, entity, Box::new(ElseFreezeState{ child_status: NodeStatus::Beginning }))
    }

    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let state = Self::downcast(state).expect("Invalid state.");
        if !self.check(world, entity) {
            return NodeStatus::Pending(Box::new(state));
        }
        let child_status = match state.child_status {
            NodeStatus::Beginning => {
                self.child.begin(world, entity)
            },
            NodeStatus::Pending(child_state) => {
                self.child.resume(world, entity, child_state)
            },
            NodeStatus::Complete(_) => {panic!("Invalid child status.")},
        };
        match &child_status {
            NodeStatus::Beginning => {panic!("Invalid child status.")},
            NodeStatus::Pending(_) => {
                NodeStatus::Pending(Box::new(ElseFreezeState { child_status }))
            },
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
#[derive(NodeState)]
struct ElseFreezeState {
    child_status: NodeStatus,
}




#[cfg(test)]
mod tests {
    use crate::tester_util::prelude::*;
    use bevy::state::app::StatesPlugin;

    #[derive(Component)]
    struct TestMarker;

    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Default, States)]
    enum TestStates {
        #[default]
        MainState,
        FreezeState,
    }

    fn test_marker_exists(In(entity): In<Entity>, world: &World) -> bool {
        world.entity(entity).contains::<TestMarker>()
    }

    #[test]
    fn test_repeat_count() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let repeater = ConditionalLoop::new(
            task,
            |In((_, loop_state)): In<(Entity, LoopState)>| loop_state.count < 3
        );
        let _entity = app.world_mut().spawn(BehaviorTreeBundle::from_root(repeater)).id();
        app.update();
        app.update();  // 0
        app.update();  // 1
        app.update();  // 2, repeater complete
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 3},
        ]};
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "ConditionalLoop should repeat the task. found: {:?}", found
        );
    }

    #[test]
    fn test_check_if_false() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = CheckIf::new(test_marker_exists);
        let entity = app.world_mut().spawn(BehaviorTreeBundle::from_root(task)).id();
        app.update();
        app.update();
        let tree_status = app.world().get::<TreeStatus>(entity);
        assert!(
            match tree_status {
                Some(&TreeStatus(NodeStatus::Complete(NodeResult::Failure))) => true,
                _ => false
            },
            "CheckIf should match the result."
        );
    }

    #[test]
    fn test_check_if_true() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = CheckIf::new(test_marker_exists);
        let entity = app.world_mut().spawn((BehaviorTreeBundle::from_root(task), TestMarker)).id();
        app.update();
        app.update();
        let tree_status = app.world().get::<TreeStatus>(entity);
        assert!(
            match tree_status {
                Some(&TreeStatus(NodeStatus::Complete(NodeResult::Success))) => true,
                _ => false
            },
            "CheckIf should match the result."
        );
    }

    #[test]
    fn test_conditional_freeze() {
        let mut app = App::new();
        app.add_plugins((StatesPlugin, BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(2, NodeResult::Success);
        let root = ElseFreeze::new(
            task,
            |In(_), state: Res<State<TestStates>>| *state.get() == TestStates::MainState,
        );
        let _entity = app.world_mut().spawn(BehaviorTreeBundle::from_root(root)).id();
        app.init_state::<TestStates>();
        app.update();
        app.update();  // 0
        app.world_mut().get_resource_mut::<NextState<TestStates>>().unwrap().set(TestStates::FreezeState);
        app.update();  // 1
        app.update();  // 2
        app.world_mut().get_resource_mut::<NextState<TestStates>>().unwrap().set(TestStates::MainState);
        app.update();  // 3, repeater complete
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 0, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 2, frame: 3},
            TestLogEntry {task_id: 0, updated_count: 3, frame: 4},
        ]};
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "ElseFreeze should match the result. found: {:?}", found
        );
    }
}



