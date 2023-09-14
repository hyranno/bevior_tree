
use std::sync::{Arc, Mutex};
use bevy::{prelude::*, ecs::system::{ReadOnlySystemParam, SystemParam, SystemState}};
use genawaiter::sync::{Gen, Co};

use super::{Node, NodeGen, NodeResult, ResumeSignal, nullable_access::NullableWorldAccess};

pub enum TaskState {
    Running,
    Success,
    Failure,
}

pub trait Task: Send + Sync {
    type Checker: TaskChecker;
    fn task_impl(&self) -> Arc<TaskImpl<Self::Checker>>;
}
impl<T> Node for T
where
    T: Task
{
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.task_impl().run(world, entity)
    }
}


pub struct TaskImpl<Checker>
where
    Checker: TaskChecker,
{
    checker: Checker,
    system_state: Mutex<Option<SystemState<Checker::Param<'static, 'static>>>>,
    on_enter: Vec<Box<dyn Fn(Entity, Commands) + Send + Sync>>,
    on_exit: Vec<Box<dyn Fn(Entity, Commands) + Send + Sync>>,
}
impl<Checker> TaskImpl<Checker>
where
    Checker: TaskChecker,
{
    pub fn new(
        checker: Checker,
    ) -> Self {
        Self {
            checker,
            system_state: Mutex::new(None),
            on_enter: vec![],
            on_exit: vec![],
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
}
impl<Checker> Node for TaskImpl<Checker>
where
    Checker: TaskChecker,
{
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co: Co<(), ResumeSignal>| async move {
            for event in self.on_enter.iter() {
                world.lock().unwrap().entity_command_call(entity, &event).unwrap();
            }
            let mut result: Option<NodeResult> = None;
            while result.is_none() {
                let task_state = world.lock().unwrap().check_task(
                    entity,
                    &self.checker,
                    self.system_state.lock().as_mut().unwrap()
                ).unwrap();
                match task_state {
                    TaskState::Running => {
                        let signal = co.yield_(()).await;
                        if signal == ResumeSignal::Abort {
                            result = Some(NodeResult::Aborted);
                        }
                    },
                    TaskState::Success => {
                        result = Some(NodeResult::Success);
                        // TODO on_success
                    },
                    TaskState::Failure => {
                        result = Some(NodeResult::Failure);
                        // TODO on_failure
                    },
                }
            }
            for event in self.on_exit.iter() {
                world.lock().unwrap().entity_command_call(entity, &event).unwrap();
            }
            result.unwrap()
        };
        Box::new(Gen::new(producer))
    }
}


pub trait TaskChecker: 'static + Sized + Send + Sync {
    type Param<'w, 's>: ReadOnlySystemParam;
    fn check (
        &self,
        entity: Entity,
        param: <<Self as TaskChecker>::Param<'_, '_> as SystemParam>::Item<'_, '_>,
    ) -> TaskState;
}
