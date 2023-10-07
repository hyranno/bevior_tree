//! Decorator nodes for conditional flow control.

use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::{ecs::entity::Entity, prelude::{ReadOnlySystem, IntoSystem}};

use super::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};


pub mod variants;


/// Node that repeat the child while condition is matched.
pub struct ConditionalLoop {
    child: Arc<dyn Node>,
    checker: Mutex<Box<dyn ReadOnlySystem<In=(Entity, u32, Option<NodeResult>), Out=bool>>>,
}
impl ConditionalLoop {
    pub fn new<F, Marker>(child: Arc<dyn Node>, checker: F) -> Arc<Self>
    where
        F: IntoSystem<(Entity, u32, Option<NodeResult>), bool, Marker>,
        <F as IntoSystem<(Entity, u32, Option<NodeResult>), bool, Marker>>::System : ReadOnlySystem,
    {
        Arc::new(Self {
            child,
            checker: Mutex::new(Box::new(IntoSystem::into_system(checker))),
        })
    }

    fn check(&self, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity, loop_count: u32, last_result: Option<NodeResult>) -> bool {
        world.lock().unwrap().check_loop_condition(
            entity,
            self.checker.lock().as_deref_mut().unwrap(),
            loop_count, last_result,
        ).unwrap()
    }
}
impl Node for ConditionalLoop {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co| async move {
            let mut last_result: Option<NodeResult> = None;
            let mut loop_count = 0;
            while self.check(world.clone(), entity, loop_count, last_result) {
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

