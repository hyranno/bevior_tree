//! Node that represents Task.

use std::{sync::Mutex, vec};

use bevy::ecs::{
    bundle::Bundle,
    entity::Entity,
    system::{Commands, In, IntoSystem, ReadOnlySystem, System},
    world::World,
};

use crate::node::prelude::*;

pub mod prelude {
    pub use super::{TaskBridge, TaskDefinition, TaskEventListener, TaskChecker, insert_while_running, TaskEvent, TaskStatus};
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is running and its result is pending, so not ready to proceed next node.
    Running,
    /// Task is complete with result, ready to proceed next node.
    Complete(NodeResult),
}

/// State for [`TaskBridge`]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(NodeState, Debug)]
struct TaskState;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEvent {
    Enter,
    Exit,
    Success,
    Failure,
}


pub trait TaskChecker: ReadOnlySystem<In = In<Entity>, Out = TaskStatus> {}
impl<S> TaskChecker for S where S: ReadOnlySystem<In = In<Entity>, Out = TaskStatus> {}

pub trait TaskEventListener: System<In = In<Entity>, Out = ()> {}
impl<S> TaskEventListener for S where S: System<In = In<Entity>, Out = ()> {}

#[cfg_attr(feature = "serde", typetag::serde(tag = "type"))]
pub trait TaskDefinition: 'static + Send + Sync {
    fn build_checker(&self) -> Box<dyn TaskChecker>;
    fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<dyn TaskEventListener>)>;
}

/// Event listeners that add the bundle on entering node then remove it on exiting.
pub fn insert_while_running<T: Bundle + 'static + Clone>(bundle: T) -> Vec<(TaskEvent, Box<dyn TaskEventListener>)> {
    vec![
        (
            TaskEvent::Enter,
            Box::new(IntoSystem::into_system(move |In(entity), mut commands: Commands| {
                commands.entity(entity).insert(bundle.clone());
            }))
        ),
        (
            TaskEvent::Exit,
            Box::new(IntoSystem::into_system(|In(entity), mut commands: Commands| {
                commands.entity(entity).remove::<T>();
            }))
        ),
    ]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[with_state(TaskState)]
pub struct TaskBridge {
    definition: Box<dyn TaskDefinition>,
    #[cfg_attr(feature = "serde", serde(skip))]
    checker: Mutex<Option<Box<dyn TaskChecker>>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    event_listeners: Mutex<Vec<(TaskEvent, Box<dyn TaskEventListener>)>>,
}
impl TaskBridge {
    pub fn new(definition: Box<dyn TaskDefinition>) -> Self {
        Self {
            definition,
            checker: Mutex::new(None),
            event_listeners: Mutex::new(vec![]),
        }
    }

    /// Check current [`TaskStatus`].
    fn check(&self, world: &mut World, entity: Entity) -> TaskStatus {
        let mut checker = self.checker.lock().expect("Failed to lock.");
        // Initialize checker if not yet.
        if checker.is_none() {
            let mut built_checker = self.definition.build_checker();
            built_checker.initialize(world);
            *checker = Some(built_checker);
        }
        checker
            .as_mut()
            .expect("Checker should be some here.")
            .run_readonly(entity, world)
            .expect("Failed to run checker system.")
    }

    fn trigger_event(&self, world: &mut World, entity: Entity, event: TaskEvent) {
        let mut listeners = self.event_listeners.lock().expect("Failed to lock.");
        // Initialize event listeners if not yet.
        if listeners.is_empty() {
            let mut built_listeners = self.definition.build_event_listeners();
            built_listeners
                .iter_mut()
                .for_each(|(_, sys)| {sys.initialize(world);});
            *listeners = built_listeners;
        }
        listeners
            .iter_mut()
            .filter(|(ev, _)| *ev == event)
            .for_each(|(_, sys)| {
                sys.run(entity, world).expect("Failed to run event system.");
                sys.apply_deferred(world);
            });
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
            }
        }
    }
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        let _state = Self::downcast(state).expect("Invalid state.");
        self.trigger_event(world, entity, TaskEvent::Exit);
    }
}
