//! Node that represents Task.

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

/// State for [`TaskBridge`]
#[derive(NodeState, Debug)]
struct TaskState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEvent {
    Enter,
    Exit,
    Success,
    Failure,
}


/// Node that represents task.
/// This node is for telling what to do for other systems.
/// 
/// This node marks what to do for other systems (usually via adding component) and check the completion to proceed the behavior tree.
/// [`TaskBridge::insert_while_running`] is good for marking.
///
/// This pattern is needed to keep advantages of ECS.
/// ECS does good performance running same kind of tasks in a batch.
/// But while processing the behavior trees, various tasks appears in various order.
/// So the this nodes just marks what to do, expecting other systems does actual updates later.
#[with_state(TaskState)]
pub struct TaskBridge {
    checker: Mutex<Box<dyn ReadOnlySystem<In=In<Entity>, Out=TaskStatus>>>,
    event_listeners: Mutex<Vec<(TaskEvent, Box<dyn System<In=In<Entity>, Out=()>>)>>,
}
impl TaskBridge {
    pub fn new<F, Marker>(checker: F) -> TaskBridge
    where
        F: IntoSystem<In<Entity>, TaskStatus, Marker>,
        <F as IntoSystem<In<Entity>, TaskStatus, Marker>>::System : ReadOnlySystem,
    {
        TaskBridge {
            checker: Mutex::new(Box::new(IntoSystem::into_system(checker))),
            event_listeners: Mutex::new(vec![]),
        }
    }
    /// Register callback for [`TaskEvent`].
    /// Use this to communicate to bevy world.
    pub fn on_event<Marker>(self, event: TaskEvent, callback: impl IntoSystem<In<Entity>, (), Marker>) -> Self {
        self.event_listeners.lock().expect("Failed to lock.").push((event, Box::new(IntoSystem::into_system(callback))));
        self
    }
    /// Register callbacks that add the bundle on entering node then remove it on exiting.
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

