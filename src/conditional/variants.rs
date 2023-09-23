use std::sync::{Arc, Mutex};

use bevy::ecs::{entity::Entity, system::{ReadOnlySystemParam, SystemParam, SystemState,}};
use genawaiter::sync::Gen;

use crate::{Node, NodeGen, NodeResult, nullable_access::NullableWorldAccess};
use super::{ConditionChecker, ConditionalLoop};


/// Node that runs the child if condition is matched.
pub struct Conditional<Checker: EcsConditionChecker> {
    delegate: Arc<ConditionalLoop<SeparableConditionChecker<Checker, RepeatCount>>>,
}
impl<Checker: EcsConditionChecker> Conditional<Checker> {
    pub fn new(child: Arc<dyn Node>, checker: Checker) -> Arc<Self> {
        Arc::new(Self { delegate: ConditionalLoop::new(
            child,
            SeparableConditionChecker::new(checker, RepeatCount { count: 1 })
        ) })
    }
}
impl<Checker: EcsConditionChecker> Node for Conditional<Checker> {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}

/// Node that check the condition, then return it as `NodeResult`.
pub struct CheckIf<Checker: EcsConditionChecker> {
    checker: SeparableConditionChecker<Checker, Always>,
    system_state: Mutex<Option<SystemState<Checker::Param<'static, 'static>>>>,
}
impl<Checker: EcsConditionChecker> CheckIf<Checker> {
    pub fn new(checker: Checker) -> Arc<Self> {
        Arc::new(Self {
            checker: SeparableConditionChecker::new(checker, Always),
            system_state: Mutex::new(None),
        })
    }
}
impl<Checker: EcsConditionChecker> Node for CheckIf<Checker> {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |_| async move {
            world.lock().unwrap().check_condition(
                entity,
                &self.checker,
                &mut self.system_state.lock().unwrap(),
                0, None
            ).unwrap().into()
        };
        Box::new(Gen::new(producer))
    }
}


/// `ConditionChecker` consists of `EcsConditionChecker` and `LoopVarsConditionChecker`.
pub struct SeparableConditionChecker<EcsChecker, LoopVarsChecker>
where
    EcsChecker: EcsConditionChecker,
    LoopVarsChecker: LoopVarsConditionChecker
{
    ecs_checker: EcsChecker,
    loop_var_checker: LoopVarsChecker,
}
impl<E, L> SeparableConditionChecker<E, L>
where
    E: EcsConditionChecker,
    L: LoopVarsConditionChecker,
{
    pub fn new(ecs_checker: E, loop_var_checker: L) -> Self {
        Self { ecs_checker, loop_var_checker }
    }
}
impl<E, L> EcsConditionChecker for SeparableConditionChecker<E, L>
where
    E: EcsConditionChecker,
    L: LoopVarsConditionChecker,
{
    type Param<'w, 's> = E::Param<'w, 's>;
    fn check_params(
        &self,
        entity: Entity,
        param: <<Self as EcsConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
    ) -> bool {
        self.ecs_checker.check_params(entity, param)
    }
}
impl<E, L> LoopVarsConditionChecker for SeparableConditionChecker<E, L>
where
    E: EcsConditionChecker,
    L: LoopVarsConditionChecker,
{
    fn check_loop_vars(&self, loop_count: u32, last_result: Option<NodeResult>) -> bool {
        self.loop_var_checker.check_loop_vars(loop_count, last_result)
    }
}

/// Check condition with ecs world.
pub trait EcsConditionChecker: 'static + Sized + Send + Sync {
    type Param<'w, 's>: ReadOnlySystemParam;
    fn check_params(
        &self,
        entity: Entity,
        param: <<Self as EcsConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
    ) -> bool;
}
/// Check condition with the loop count and the last result.
pub trait LoopVarsConditionChecker: 'static + Sized + Send + Sync {
    fn check_loop_vars(&self, loop_count: u32, last_result: Option<NodeResult>) -> bool;
}
impl<Checker> ConditionChecker for Checker
where
    Checker: EcsConditionChecker + LoopVarsConditionChecker
{
    type Param<'w, 's> = <Checker as EcsConditionChecker>::Param<'w, 's>;
    fn check (
        &self,
        entity: Entity,
        param: <<Self as ConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
        loop_count: u32,
        last_result: Option<NodeResult>,
    ) -> bool {
        self.check_loop_vars(loop_count, last_result) && self.check_params(entity, param)
    }
}


pub struct Always;
impl EcsConditionChecker for Always {
    type Param<'w, 's> = ();
    fn check_params(
        &self,
        _entity: Entity,
        _param: <<Self as EcsConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
    ) -> bool {
        true
    }
}
impl LoopVarsConditionChecker for Always {
    fn check_loop_vars(&self, _loop_count: u32, _last_result: Option<NodeResult>) -> bool {
        true
    }
}


/// Returns true until given count.
pub struct RepeatCount {
    pub count: u32,
}
impl LoopVarsConditionChecker for RepeatCount {
    fn check_loop_vars(&self, loop_count: u32, _last_result: Option<NodeResult>) -> bool {
        loop_count < self.count
    }
}
impl ConditionChecker for RepeatCount {
    type Param<'w, 's> = ();
    fn check (
        &self,
        _entity: Entity,
        _param: <<Self as ConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
        loop_count: u32,
        last_result: Option<NodeResult>,
    ) -> bool {
        self.check_loop_vars(loop_count, last_result)
    }
}

/// Returns true until the child return the given result.
pub struct UntilResult {
    pub until: NodeResult,
}
impl LoopVarsConditionChecker for UntilResult {
    fn check_loop_vars(&self, _loop_count: u32, last_result: Option<NodeResult>) -> bool {
        match last_result {
            None => true,
            Some(result) => result != self.until,
        }
    }
}
impl ConditionChecker for UntilResult {
    type Param<'w, 's> = ();
    fn check (
        &self,
        _entity: Entity,
        _param: <<Self as ConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
        loop_count: u32,
        last_result: Option<NodeResult>,
    ) -> bool {
        self.check_loop_vars(loop_count, last_result)
    }
}




#[cfg(test)]
mod tests {
    use crate::tester_util::*;

    #[derive(Component)]
    struct TestMarker;

    struct TestMarkerExists;
    impl EcsConditionChecker for TestMarkerExists {
        type Param<'w, 's> = Query<'w, 's, &'static TestMarker>;
        fn check_params(
            &self,
            entity: Entity,
            param: <<Self as EcsConditionChecker>::Param<'_, '_> as bevy::ecs::system::SystemParam>::Item<'_, '_>,
        ) -> bool {
            param.get(entity).is_ok()
        }
    }

    #[test]
    fn test_conditional_false() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let conditional = Conditional::new(task, TestMarkerExists);
        let tree = BehaviorTree::new(conditional);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // nop
        let expected = TestLog {log: vec![
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "Conditional should not do the task."
        );
    }

    #[test]
    fn test_conditional_true() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let conditional = Conditional::new(task, TestMarkerExists);
        let tree = BehaviorTree::new(conditional);
        let _entity = app.world.spawn((tree, TestMarker)).id();
        app.update();
        app.update();  // 0
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "Conditional should do the task."
        );
    }

    #[test]
    fn test_check_if_false() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = CheckIf::new(TestMarkerExists);
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
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = CheckIf::new(TestMarkerExists);
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
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let repeater = ConditionalLoop::new(task, RepeatCount {count: 3});
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

}
