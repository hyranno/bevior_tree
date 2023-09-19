//! Tools for bypassing lifetime check.
//!
//! This module is kind of unsafe.
//! Use with extra caution.

use std::sync::{Arc, Mutex};

use bevy::{prelude::*, ecs::system::SystemState};

use crate::{decorator::ConditionChecker, NodeResult, sequencial::Scorer};

use super::task::{TaskState, TaskChecker};

pub struct NullableWorldAccess {
    ptr: Option<&'static mut World>,
    command_system_state: Option<SystemState<Commands<'static, 'static>>>,
}
impl NullableWorldAccess {
    pub fn check_task<Checker>(&mut self, entity: Entity, checker: &Checker, system_state: &mut Option<SystemState<Checker::Param<'static, 'static>>>)
        -> Result<TaskState, NullableAccessError>
    where
        Checker: TaskChecker,
    {
        let Some(world) = self.ptr.as_deref_mut() else {
            return Err(NullableAccessError::NotAvailableNow);
        };
        if system_state.is_none() {
            *system_state = Some(SystemState::new(world));
        }
        let param = system_state.as_mut().unwrap().get(world);
        Ok(checker.check(entity, param))
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
        let Some(world) = self.ptr.as_deref_mut() else {
            return Err(NullableAccessError::NotAvailableNow);
        };
        if system_state.is_none() {
            *system_state = Some(SystemState::new(world));
        }
        let param = system_state.as_mut().unwrap().get(world);
        Ok(checker.check(entity, param, loop_count, last_result))
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
        let Some(world) = self.ptr.as_deref_mut() else {
            return Err(NullableAccessError::NotAvailableNow);
        };
        if system_state.is_none() {
            *system_state = Some(SystemState::new(world));
        }
        let param = system_state.as_mut().unwrap().get(world);
        Ok(scorer.score(entity, param))
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
        let Some(system_state) = self.command_system_state.as_mut() else {
            return Err(NullableAccessError::SystemStateUnavailable);
        };
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
    SystemStateUnavailable,
}

pub struct TemporalWorldSharing {
    accessor: Arc<Mutex<NullableWorldAccess>>,
}
impl TemporalWorldSharing {
    pub fn new(accessor: Arc<Mutex<NullableWorldAccess>>, value: &mut World) -> Self {
        let ptr: *mut World = value;
        let prolonged_ref: &'static mut World = unsafe{ ptr.as_mut().unwrap() };
        accessor.lock().unwrap().ptr = Some(prolonged_ref);
        Self {accessor}
    }
}
impl Drop for TemporalWorldSharing {
    fn drop(&mut self) {
        self.accessor.lock().unwrap().ptr = None;
    }
}