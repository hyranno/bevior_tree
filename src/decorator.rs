
use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::ecs::{entity::Entity, system::{ReadOnlySystemParam, SystemParam, SystemState}};

use super::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};



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


pub struct ConditionalLoop<Checker: ConditionChecker> {
    child: Arc<dyn Node>,
    checker: Checker,
    system_state: Mutex<Option<SystemState<Checker::Param<'static, 'static>>>>,
}
impl<Checker: ConditionChecker> ConditionalLoop<Checker> {
    pub fn new(child: Arc<dyn Node>, checker: Checker) -> Self {
        Self { child, checker, system_state: Mutex::new(None) }
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

