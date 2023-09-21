//! Decorator nodes.

use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::ecs::{entity::Entity, system::{ReadOnlySystemParam, SystemParam, SystemState}};

use super::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};


/// Check condition for conditional node.
pub trait ConditionChecker: 'static + Sized + Send + Sync {
    type Param<'w, 's>: ReadOnlySystemParam;
    fn check (
        &self,
        entity: Entity,
        param: <<Self as ConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
        loop_count: u32,
        last_result: Option<NodeResult>,
    ) -> bool;
}


/// Node that repeat subnode while condition is matched.
pub struct ConditionalLoop<Checker: ConditionChecker> {
    child: Arc<dyn Node>,
    checker: Checker,
    system_state: Mutex<Option<SystemState<Checker::Param<'static, 'static>>>>,
}
impl<Checker: ConditionChecker> ConditionalLoop<Checker> {
    pub fn new(child: Arc<dyn Node>, checker: Checker) -> Arc<Self> {
        Arc::new(Self { child, checker, system_state: Mutex::new(None) })
    }
}
impl<Checker: ConditionChecker> Node for ConditionalLoop<Checker> {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co| async move {
            let mut last_result: Option<NodeResult> = None;
            let mut loop_count = 0;
            while world.lock().unwrap().check_condition(
                entity, &self.checker, self.system_state.lock().as_mut().unwrap(), loop_count, last_result,
            ).unwrap() {
                let mut gen = self.child.clone().run(world.clone(), entity);
                let node_result = complete_or_yield(&co, &mut gen).await;
                match node_result {
                    NodeResult::Aborted => { return node_result; },
                    _ => {},
                }
                last_result = Some(node_result);
                loop_count += 1;
            }
            if let Some(result) = last_result {
                result
            } else {
                NodeResult::Failure
            }
        };
        Box::new(Gen::new(producer))
    }
}

pub struct Always;
impl ConditionChecker for Always {
    type Param<'w, 's> = ();
    fn check (
        &self,
        _entity: Entity,
        _param: <<Self as ConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
        _loop_count: u32,
        _last_result: Option<NodeResult>,
    ) -> bool {
        true
    }
}

/// Returns true until given count.
pub struct RepeatCount {
    pub count: u32,
}
impl ConditionChecker for RepeatCount {
    type Param<'w, 's> = ();
    fn check (
        &self,
        _entity: Entity,
        _param: <<Self as ConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
        loop_count: u32,
        _last_result: Option<NodeResult>,
    ) -> bool {
        loop_count < self.count
    }
}

/// Returns true until the subnode return given result.
pub struct UntilResult {
    pub until: NodeResult,
}
impl ConditionChecker for UntilResult {
    type Param<'w, 's> = ();
    fn check (
        &self,
        _entity: Entity,
        _param: <<Self as ConditionChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
        _loop_count: u32,
        last_result: Option<NodeResult>,
    ) -> bool {
        if let Some(result) = last_result {
            result != self.until
        } else {
            true
        }
    }
}


/// Node that converts result of subnode.
pub struct ResultConverter<F>
where
F: Fn(bool) -> bool + 'static + Send + Sync,
{
    child: Arc<dyn Node>,
    convert: F,
}
impl<F> ResultConverter<F>
where
    F: Fn(bool) -> bool + 'static + Send + Sync,
{
    pub fn new(child: Arc<dyn Node>, convert: F) -> Arc<Self> {
        Arc::new(Self { child, convert })
    }
}
impl<F> Node for ResultConverter<F>
where
    F: Fn(bool) -> bool + 'static + Send + Sync,
{
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co| async move {
            let mut gen = self.child.clone().run(world.clone(), entity);
            let node_result = complete_or_yield(&co, &mut gen).await;
            match node_result {
                NodeResult::Aborted => { node_result },
                _ => { (self.convert)(node_result.into()).into() },
            }
        };
        Box::new(Gen::new(producer))
    }
}


#[cfg(test)]
mod tests {
    use crate::tester_util::*;

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
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "ConditionalLoop should repeat the task."
        );
    }

    #[test]
    fn test_invert_result() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let inverter = ResultConverter::new(task, |res| !res);
        let tree = BehaviorTree::new(inverter);
        let entity = app.world.spawn(tree).id();
        app.update();
        app.update();
        let tree = app.world.get::<BehaviorTree>(entity).unwrap();
        assert!(
            tree.result.unwrap() == NodeResult::Failure,
            "ResultConverter should change the result."
        );
    }
}
