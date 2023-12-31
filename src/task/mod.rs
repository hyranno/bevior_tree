//! Task, the leaf nodes of the trees.
//! 
//! Task node does not directly run your task.
//! It can do something on enter or exit, checking the completion of the task every frame while running.
//! Typically, it adds and removes some components to the entity.
//! You need some system to update according to the components.

use std::sync::{Arc, Mutex};
use bevy::prelude::*;
use genawaiter::sync::{Gen, Co};

use super::{Node, NodeGen, NodeResult, ResumeSignal, nullable_access::NullableWorldAccess};


pub mod variants;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Running,
    Success,
    Failure,
}

/// Implement this for your task node.
pub trait Task: Send + Sync {
    fn task_impl(&self) -> Arc<TaskImpl>;
}
impl<T> Node for T
where
    T: Task
{
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.task_impl().run(world, entity)
    }
}


/// Core implementation of task node.
/// You can directly use this as a task node for simple task.
pub struct TaskImpl {
    checker: Mutex<Box<dyn ReadOnlySystem<In=Entity, Out=TaskState>>>,
    on_enter: Vec<Box<dyn Fn(Entity, Commands) + Send + Sync>>,
    on_exit: Vec<Box<dyn Fn(Entity, Commands) + Send + Sync>>,
    on_success: Vec<Box<dyn Fn(Entity, Commands) + Send + Sync>>,
    on_failure: Vec<Box<dyn Fn(Entity, Commands) + Send + Sync>>,
}
impl TaskImpl {
    pub fn new<F, Marker>(checker: F) -> TaskImpl
    where
        F: IntoSystem<Entity, TaskState, Marker>,
        <F as IntoSystem<Entity, TaskState, Marker>>::System : ReadOnlySystem,
    {
        TaskImpl {
            checker: Mutex::new(Box::new(IntoSystem::into_system(checker))),
            on_enter: vec![],
            on_exit: vec![],
            on_success: vec![],
            on_failure: vec![],
        }
    }
    pub fn on_enter(
        mut self,
        callback: impl 'static + Fn(Entity, Commands) + Send + Sync,
    ) -> Self {
        self.on_enter.push(Box::new(callback));
        self
    }
    pub fn on_exit(
        mut self,
        callback: impl 'static + Fn(Entity, Commands) + Send + Sync,
    ) -> Self {
        self.on_exit.push(Box::new(callback));
        self
    }
    pub fn on_success(
        mut self,
        callback: impl 'static + Fn(Entity, Commands) + Send + Sync,
    ) -> Self {
        self.on_success.push(Box::new(callback));
        self
    }
    pub fn on_failure(
        mut self,
        callback: impl 'static + Fn(Entity, Commands) + Send + Sync,
    ) -> Self {
        self.on_failure.push(Box::new(callback));
        self
    }    
    /// Insert the bundle on enter the task, then remove it on exit.
    pub fn insert_while_running<T: Bundle + 'static + Clone>(
        self,
        bundle: T,
    ) -> Self {
        self
            .on_enter(Box::new(move |entity, mut commands: Commands| {
                commands.entity(entity).insert(bundle.clone());
            }))
            .on_exit(Box::new(|entity, mut commands: Commands| {
                commands.entity(entity).remove::<T>();
            }))
    }
    /// Check current [`TaskState`].
    fn check(&self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> TaskState {
        world.lock().unwrap().check_task(
            entity,
            self.checker.lock().as_deref_mut().unwrap(),
        ).unwrap()
    }
    fn trigger_enter(&self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) {
        for event in self.on_enter.iter() {
            world.lock().unwrap().entity_command_call(entity, &event).unwrap();
        }
    }
    fn trigger_exit(&self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) {
        // If aborted with dropping BehaviorTree, world will not be accessible.
        #[allow(unused_must_use)]
        for event in self.on_exit.iter() {
            world.lock().unwrap().entity_command_call(entity, &event);
        }
    }
    fn trigger_success(&self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) {
        for event in self.on_success.iter() {
            world.lock().unwrap().entity_command_call(entity, &event).unwrap();
        }
    }
    fn trigger_failure(&self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) {
        for event in self.on_failure.iter() {
            world.lock().unwrap().entity_command_call(entity, &event).unwrap();
        }
    }
}
impl Node for TaskImpl {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co: Co<(), ResumeSignal>| async move {
            self.trigger_enter(world.clone(), entity);
            let mut result: Option<NodeResult> = None;
            while result.is_none() {
                match self.check(world.clone(), entity) {
                    TaskState::Running => {
                        let signal = co.yield_(()).await;
                        if signal == ResumeSignal::Abort {
                            result = Some(NodeResult::Aborted);
                        }
                    },
                    TaskState::Success => {
                        result = Some(NodeResult::Success);
                        self.trigger_success(world.clone(), entity);
                    },
                    TaskState::Failure => {
                        result = Some(NodeResult::Failure);
                        self.trigger_failure(world.clone(), entity);
                    },
                }
            }
            self.trigger_exit(world.clone(), entity);
            result.unwrap()
        };
        Box::new(Gen::new(producer))
    }
}


