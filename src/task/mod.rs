
use std::{sync::{Arc, Mutex}, any::Any};

use bevy::ecs::{world::World, system::{ReadOnlySystem, System, IntoSystem, Commands, In}, entity::Entity, bundle::Bundle};

use crate::node::{Node, NodeResult, NodeState, NodeStatus};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Complete(NodeResult),
}

#[derive(Debug)]
struct TaskState;
impl NodeState for TaskState {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEvent {
    Enter,
    Exit,
    Success,
    Failure,
}

pub struct TaskBridge {
    checker: Mutex<Box<dyn ReadOnlySystem<In=Entity, Out=TaskStatus>>>,
    event_listeners: Mutex<Vec<(TaskEvent, Box<dyn System<In=Entity, Out=()>>)>>,
}
impl TaskBridge {
    pub fn new<F, Marker>(checker: F) -> Arc<TaskBridge>
    where
        F: IntoSystem<Entity, TaskStatus, Marker>,
        <F as IntoSystem<Entity, TaskStatus, Marker>>::System : ReadOnlySystem,
    {
        Arc::new(TaskBridge {
            checker: Mutex::new(Box::new(IntoSystem::into_system(checker))),
            event_listeners: Mutex::new(vec![]),
        })
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
    fn check(&self, world: &World, entity: Entity) -> TaskStatus {
        self.checker.lock().expect("Failed to lock.").run_readonly(entity, world)
    }

    fn trigger_event(&self, world: &mut World, entity: Entity, event: TaskEvent) {
        self.event_listeners.lock().expect("Failed to lock.").iter_mut().filter(|(ev, _)| *ev == event).for_each(
            |(_, sys)| sys.run(entity, world)
        );
    }
}
impl Node for TaskBridge {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.trigger_event(world, entity, TaskEvent::Enter);
        self.resume(world, entity, Box::new(TaskState))
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        let state = state.into_any().downcast::<TaskState>().expect("invalid state type");
        let status = self.check(world, entity);
        match status {
            TaskStatus::Running => NodeStatus::Pending(state),
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
}

