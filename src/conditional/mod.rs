//! Decorator nodes for conditional flow control.

use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::ecs::{entity::Entity, system::{ReadOnlySystemParam, SystemParam, SystemState}};

use super::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};


pub mod variants;

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


/// Node that repeat the child while condition is matched.
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

