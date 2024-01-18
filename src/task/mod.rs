
use std::sync::Mutex;

use bevy::ecs::{world::World, system::{ReadOnlySystem, System, IntoSystem, Commands, In}, entity::Entity, bundle::Bundle};

use crate::node::prelude::*;

pub mod prelude {
    pub use super::{TaskBridge, TaskEvent, TaskStatus,};
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Complete(NodeResult),
}

#[derive(NodeState, Debug)]
struct TaskState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEvent {
    Enter,
    Exit,
    Success,
    Failure,
}

#[with_state(TaskState)]
pub struct TaskBridge {
    checker: Mutex<Box<dyn ReadOnlySystem<In=Entity, Out=TaskStatus>>>,
    event_listeners: Mutex<Vec<(TaskEvent, Box<dyn System<In=Entity, Out=()>>)>>,
}
impl TaskBridge {
    pub fn new<F, Marker>(checker: F) -> TaskBridge
    where
        F: IntoSystem<Entity, TaskStatus, Marker>,
        <F as IntoSystem<Entity, TaskStatus, Marker>>::System : ReadOnlySystem,
    {
        TaskBridge {
            checker: Mutex::new(Box::new(IntoSystem::into_system(checker))),
            event_listeners: Mutex::new(vec![]),
        }
    }
    pub fn on_event<Marker>(self, event: TaskEvent, callback: impl IntoSystem<Entity, (), Marker>) -> Self {
        self.event_listeners.lock().expect("Failed to lock.").push((event, Box::new(IntoSystem::into_system(callback))));
        self
    }
    pub fn insert_while_running<T: Bundle + 'static + Clone>(self, bundle: T) -> Self {
        self
            .on_event(TaskEvent::Enter, move |In(entity), mut commands: Commands| {
                commands.entity(entity).insert(bundle.clone());
            })
            .on_event(TaskEvent::Exit, |In(entity), mut commands: Commands| {
                commands.entity(entity).remove::<T>();
            })
    }

    /// Check current [`TaskStatus`].
    fn check(&self, world: &mut World, entity: Entity) -> TaskStatus {
        let mut checker = self.checker.lock().expect("Failed to lock.");
        checker.initialize(world);
        checker.run_readonly(entity, world)
    }

    fn trigger_event(&self, world: &mut World, entity: Entity, event: TaskEvent) {
        self.event_listeners.lock().expect("Failed to lock.").iter_mut().filter(|(ev, _)| *ev == event).for_each(
            |(_, sys)| {
                sys.initialize(world);
                sys.run(entity, world);
                sys.apply_deferred(world);
            }
        );
    }
}
impl Node for TaskBridge {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.trigger_event(world, entity, TaskEvent::Enter);
        self.resume(world, entity, Box::new(TaskState))
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let state = Self::downcast(state).expect("Invalid state.");
        let status = self.check(world, entity);
        match status {
            TaskStatus::Running => NodeStatus::Pending(Box::new(state)),
            TaskStatus::Complete(result) => {
                match result {
                    NodeResult::Success => self.trigger_event(world, entity, TaskEvent::Success),
                    NodeResult::Failure => self.trigger_event(world, entity, TaskEvent::Failure),
                }
                self.trigger_event(world, entity, TaskEvent::Exit);
                NodeStatus::Complete(result)
            },
        }
    }
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        let _state = Self::downcast(state).expect("Invalid state.");
        self.trigger_event(world, entity, TaskEvent::Exit);
    }
}

