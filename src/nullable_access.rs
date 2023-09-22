//! Tools for bypassing lifetime check.
//!
//! This module is kind of unsafe.
//! Use with extra caution.

use std::sync::{Arc, Mutex};

use bevy::{prelude::*, ecs::system::{SystemState, ReadOnlySystemParam}};

use crate::{conditional::ConditionChecker, NodeResult, sequential::Scorer};

use super::task::{TaskState, TaskChecker};

/// Async closures used for `NodeGen` may live longer than `&World`, so they cannot have that reference.
/// This class provides the access to the world with runtime check rather than lifetime restriction.
/// Never leak out the `ptr` which unsafely holds the `&'w World` as `&'static World`.
pub struct NullableWorldAccess {
    ptr: Option<&'static mut World>,
    command_system_state: Option<SystemState<Commands<'static, 'static>>>,
}
impl NullableWorldAccess {

    /// Read only access to the world.
    /// Must not leak out the `ptr`.
    /// Maybe there are ways to leak out via `R`, so this method is kept private.
    /// Call this from outside the module via specialized methods.
    fn call_read_only<Param, F, R>(&mut self, entity: Entity, system_state: &mut Option<SystemState<Param>>, f: F)
        -> Result<R, NullableAccessError>
    where
        Param: ReadOnlySystemParam,
        F: Fn(Entity, Param::Item<'_, '_>) -> R
    {
        let Some(world) = self.ptr.as_deref_mut() else {
            return Err(NullableAccessError::NotAvailableNow);
        };
        if system_state.is_none() {
            *system_state = Some(SystemState::new(world));
        }
        let param = system_state.as_mut().unwrap().get(world);
        Ok(f(entity, param))
    }

    pub fn check_task<Checker>(&mut self, entity: Entity, checker: &Checker, system_state: &mut Option<SystemState<Checker::Param<'static, 'static>>>)
        -> Result<TaskState, NullableAccessError>
    where
        Checker: TaskChecker,
    {
        self.call_read_only(entity, system_state, |e, p|
            checker.check(e, p)
        )
    }

    pub fn check_condition<Checker>(
        &mut self,
        entity: Entity,
        checker: &Checker,
        system_state: &mut Option<SystemState<Checker::Param<'static, 'static>>>,
        loop_count: u32,
        last_result: Option<NodeResult>,
    ) -> Result<bool, NullableAccessError>
    where
        Checker: ConditionChecker,
    {
        self.call_read_only(entity, system_state, |e, p|
            checker.check(e, p, loop_count, last_result)
        )
    }

    pub fn score_node<S>(
        &mut self,
        entity: Entity,
        scorer: &S,
        system_state: &mut Option<SystemState<S::Param<'static, 'static>>>,
    ) -> Result<f32, NullableAccessError>
    where
        S: Scorer,
    {
        self.call_read_only(entity, system_state, |e, p|
            scorer.score(e, p)
        )
    }

    pub fn entity_command_call(&mut self, entity: Entity, system: &(impl Fn(Entity, Commands) + Send + Sync))
        -> Result<(), NullableAccessError>
    {
        let Some(world) = self.ptr.as_deref_mut() else {
            return Err(NullableAccessError::NotAvailableNow);
        };
        if self.command_system_state.is_none() {
            self.command_system_state = Some(SystemState::new(world));
        }
        let system_state = self.command_system_state.as_mut().unwrap();
        system(entity, system_state.get(world));
        system_state.apply(world);
        Ok(())
    }
}
impl Default for NullableWorldAccess {
    fn default() -> Self {
        Self {
            ptr: None,
            command_system_state: None,
        }
    }
}
#[derive(Debug)]
pub enum NullableAccessError {
    NotAvailableNow,
}


/// `NullableWorldAccess` can access to the world while this struct is alive.
pub struct TemporalWorldSharing<'a> {
    accessor: Arc<Mutex<NullableWorldAccess>>,
    _world: &'a World,  // To ensure this struct does not live longer than the reference.
}
impl<'a> TemporalWorldSharing<'a> {
    pub fn new(accessor: Arc<Mutex<NullableWorldAccess>>, world: &'a mut World) -> Self {
        // unsafely changing lifetime `&'a` to `&'static`.
        let ptr: *mut World = world;
        let prolonged_ref: &'static mut World = unsafe{ ptr.as_mut().unwrap() };
        accessor.lock().unwrap().ptr = Some(prolonged_ref);
        Self {accessor, _world: world}
    }
}
impl<'a> Drop for TemporalWorldSharing<'a> {
    fn drop(&mut self) {
        self.accessor.lock().unwrap().ptr = None;
    }
}
