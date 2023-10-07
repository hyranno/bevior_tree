use std::{sync::{Arc, Mutex}, borrow::Cow};

use bevy::{prelude::*, ecs::system::{CombinatorSystem, Combine}};
use genawaiter::sync::{Gen, Co};

use crate::{Node, NodeGen, NodeResult, nullable_access::NullableWorldAccess, NodeRunner, NodeGenState, ResumeSignal};
use super::ConditionalLoop;


/// Node that runs the child if condition is matched.
pub struct Conditional {
    delegate: Arc<ConditionalLoop>,
}
impl Conditional {
    pub fn new<F, Marker>(child: Arc<dyn Node>, checker: F) -> Arc<Self>
    where
        F: IntoSystem<Entity, bool, Marker>,
        <F as IntoSystem<Entity, bool, Marker>>::System : ReadOnlySystem,
    {
        Arc::new(Self { delegate: ConditionalLoop::new(
            child,
            SeparableConditionChecker::new(
                IntoSystem::into_system(checker),
                IntoSystem::into_system(|In((loop_count, last_result)): In<(u32, Option<NodeResult>)>|
                    loop_count < 1 && last_result.is_none() // only once
                ),
                Cow::Borrowed("check cond")
            )
        ) })
    }
}
impl Node for Conditional {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}

/// Node that check the condition, then return it as `NodeResult`.
pub struct CheckIf {
    checker: Mutex<Box<dyn ReadOnlySystem<In=Entity, Out=bool>>>,
}
impl CheckIf {
    pub fn new<F, Marker>(checker: F) -> Arc<Self>
    where
        F: IntoSystem<Entity, bool, Marker>,
        <F as IntoSystem<Entity, bool, Marker>>::System : ReadOnlySystem,
    {
        Arc::new(Self {
            checker: Mutex::new(Box::new(
                IntoSystem::into_system(checker),
            )),
        })
    }

    fn check(&self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> bool {
        world.lock().unwrap().check_condition(
            entity,
            self.checker.lock().as_deref_mut().unwrap(),
        ).unwrap()
    }
}
impl Node for CheckIf {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |_| async move {
            self.check(world, entity).into()
        };
        Box::new(Gen::new(producer))
    }
}


/// Run the child while condition matched, else freeze.
/// Supposing to be used as a root.
pub struct ElseFreeze {
    child: Arc<dyn Node>,
    checker: Mutex<Box<dyn ReadOnlySystem<In=Entity, Out=bool>>>,
}
impl ElseFreeze {
    pub fn new<F, Marker>(child: Arc<dyn Node>, checker: F) -> Arc<Self>
    where
        F: IntoSystem<Entity, bool, Marker>,
        <F as IntoSystem<Entity, bool, Marker>>::System : ReadOnlySystem,
    {
        Arc::new(Self {
            child,
            checker: Mutex::new(Box::new(
                IntoSystem::into_system(checker),
            )),
        })
    }

    fn check(&self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> bool {
        world.lock().unwrap().check_condition(
            entity,
            self.checker.lock().as_deref_mut().unwrap(),
        ).unwrap()
    }
}
impl Node for ElseFreeze {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co: Co<(), ResumeSignal>| async move {
            while !self.check(world.clone(), entity) {
                co.yield_(()).await;
            }
            let mut runner = NodeRunner::new(self.child.clone(), world.clone(), entity);
            while *runner.state() == NodeGenState::Yielded(()) {
                co.yield_(()).await;
                if self.check(world.clone(), entity) {
                    runner.resume_if_incomplete();
                }
            }
            runner.result().unwrap()
        };
        Box::new(Gen::new(producer))
    }
}


pub type SeparableConditionChecker<A, B> = CombinatorSystem<SeparableConditionCheckerMarker, A, B>;
pub struct SeparableConditionCheckerMarker;
impl<A, B> Combine<A,B> for SeparableConditionCheckerMarker
where
    A: System<In=Entity, Out=bool>,
    B: System<In=(u32, Option<NodeResult>), Out=bool>,
{
    type In = (Entity, u32, Option<NodeResult>);
    type Out = bool;
    fn combine(
        (entity, loop_count, last_result): Self::In,
        a: impl FnOnce(<A as System>::In) -> <A as System>::Out,
        b: impl FnOnce(<B as System>::In) -> <B as System>::Out,
    ) -> Self::Out {
        a(entity) && b((loop_count, last_result))
    }
}


#[cfg(test)]
mod tests {
    use crate::tester_util::*;

    #[derive(Component)]
    struct TestMarker;

    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Default, States)]
    enum TestStates {
        #[default]
        MainState,
        FreezeState,
    }

    fn test_marker_exists(In(entity): In<Entity>, params: Query<&TestMarker>) -> bool {
        params.get(entity).is_ok()
    }

    #[test]
    fn test_conditional_false() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let conditional = Conditional::new(task, test_marker_exists);
        let tree = BehaviorTree::new(conditional);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // nop
        let expected = TestLog {log: vec![
        ]};
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Conditional should not do the task. Found {:?}", found
        );
    }

    #[test]
    fn test_conditional_true() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let conditional = Conditional::new(task, test_marker_exists);
        let tree = BehaviorTree::new(conditional);
        let _entity = app.world.spawn((tree, TestMarker)).id();
        app.update();
        app.update();  // 0
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
        ]};
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Conditional should do the task. Found {:?}", found
        );
    }

    #[test]
    fn test_check_if_false() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = CheckIf::new(test_marker_exists);
        let tree = BehaviorTree::new(task);
        let entity = app.world.spawn(tree).id();
        app.update();
        app.update();
        let tree = app.world.get::<BehaviorTree>(entity).unwrap();
        assert!(
            tree.result.unwrap() == NodeResult::Failure,
            "CheckIf should match the result."
        );
    }

    #[test]
    fn test_check_if_true() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = CheckIf::new(test_marker_exists);
        let tree = BehaviorTree::new(task);
        let entity = app.world.spawn((tree, TestMarker)).id();
        app.update();
        app.update();
        let tree = app.world.get::<BehaviorTree>(entity).unwrap();
        assert!(
            tree.result.unwrap() == NodeResult::Success,
            "CheckIf should match the result."
        );
    }

    #[test]
    fn test_repeat_count() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let repeater = ConditionalLoop::new(task, |In((_, loop_count, _))| loop_count < 3);
        let tree = BehaviorTree::new(repeater);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 0
        app.update();  // 1
        app.update();  // 2, repeater complete
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 3},
        ]};
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "ConditionalLoop should repeat the task. found: {:?}", found
        );
    }

    #[test]
    fn test_conditional_freeze() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(2, TaskState::Success);
        let root = ElseFreeze::new(
            task,
            |In(_), state: Res<State<TestStates>>| *state.get() == TestStates::MainState,
        );
        let tree = BehaviorTree::new(root);
        let _entity = app.world.spawn(tree).id();
        app.add_state::<TestStates>();
        app.update();
        app.update();  // 0
        app.world.get_resource_mut::<NextState<TestStates>>().unwrap().set(TestStates::FreezeState);
        app.update();  // 1
        app.update();  // 2
        app.world.get_resource_mut::<NextState<TestStates>>().unwrap().set(TestStates::MainState);
        app.update();  // 3, repeater complete
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 0, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 2, frame: 3},
            TestLogEntry {task_id: 0, updated_count: 3, frame: 4},
        ]};
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "ElseFreeze should match the result. found: {:?}", found
        );    }
}
