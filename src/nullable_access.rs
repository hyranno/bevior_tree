//! Tools for bypassing lifetime check.
//!
//! This module is kind of unsafe.
//! Use with extra caution.

use std::sync::{Arc, Mutex};

use bevy::{prelude::*, ecs::{system::SystemState, component::Tick}};

use crate::{NodeResult, task::TaskState};


/// Provides access to [`World`] if available.
/// 
/// Async closures used for `NodeGen` may live longer than `&World`, so they cannot have that reference.
/// This class provides the access to the world with runtime check rather than lifetime restriction.
/// Never leak out the `ptr` which unsafely holds the `&'w World` as `&'static World`.
pub struct NullableWorldAccess {
    ptr: Option<&'static mut World>,
    command_system_state: Option<SystemState<Commands<'static, 'static>>>,
}
impl NullableWorldAccess {

    pub(crate) fn new() -> Self {
        Self { ptr: None, command_system_state: None }
    }

    /// Read only access to the world.
    /// Must not leak out the `ptr`.
    /// Maybe there are ways to leak out via `R`, so this method is kept private.
    /// Call this from outside the module via specialized methods.
    fn call_read_only_sys<In: 'static, Out: 'static>(
        &mut self,
        input: In,
        sys: &mut Box<dyn ReadOnlySystem<In=In, Out=Out>>
    ) -> Result<Out, NullableAccessError> {
        match self.ptr.as_deref_mut() {
            Some(world) => {
                // While `System` does not have `is_initialized` thing, use `last_run` to check if it is initialized.
                if sys.get_last_run() == Tick::new(0) {
                    sys.initialize(world)
                }
                Ok(sys.run_readonly(input, world))
            },
            None => Err(NullableAccessError::NotAvailableNow),
        }
    }

    pub fn check_task(
        &mut self,
        entity: Entity,
        checker: &mut Box<dyn ReadOnlySystem<In=Entity, Out=TaskState>>
    ) -> Result<TaskState, NullableAccessError> {
        self.call_read_only_sys(entity, checker)
    }

    pub fn check_condition(
        &mut self,
        entity: Entity,
        checker: &mut Box<dyn ReadOnlySystem<In=Entity, Out=bool>>,
    ) -> Result<bool, NullableAccessError> {
        self.call_read_only_sys(entity, checker)
    }

    pub fn check_loop_condition(
        &mut self,
        entity: Entity,
        checker: &mut Box<dyn ReadOnlySystem<In=(Entity, u32, Option<NodeResult>), Out=bool>>,
        loop_count: u32,
        last_result: Option<NodeResult>,
    ) -> Result<bool, NullableAccessError> {
        self.call_read_only_sys((entity, loop_count, last_result), checker)
    }

    pub fn score_node(
        &mut self,
        entity: Entity,
        scorer: &mut Box<dyn ReadOnlySystem<In=Entity, Out=f32>>,
    ) -> Result<f32, NullableAccessError> {
        self.call_read_only_sys(entity, scorer)
    }

    /// Call `Fn(Entity, Commands)` using `&mut World`.
    /// Mainly for events on [`crate::task::TaskImpl`].
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
#[derive(Debug)]
pub enum NullableAccessError {
    NotAvailableNow,
}


/// [`NullableWorldAccess`] can access to the world while this struct is alive.
pub(crate) struct TemporalWorldSharing<'a> {
    accessor: Arc<Mutex<NullableWorldAccess>>,
    _world: &'a World,  // To ensure this struct does not live longer than the reference.
}
impl<'a> TemporalWorldSharing<'a> {
    pub(crate) fn new(accessor: Arc<Mutex<NullableWorldAccess>>, world: &'a mut World) -> Self {
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
