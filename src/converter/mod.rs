//! Decorator nodes that convert the result of its child.

use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::ecs::entity::Entity;

use super::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};


pub mod variants;

/// Node that converts the result of the child.
pub struct ResultConverter
where
{
    child: Arc<dyn Node>,
    convert: Box<dyn Fn(bool) -> bool + 'static + Send + Sync>,
}
impl ResultConverter {
    pub fn new(child: Arc<dyn Node>, convert: impl Fn(bool) -> bool + 'static + Send + Sync) -> Arc<Self> {
        Arc::new(Self { child, convert: Box::new(convert) })
    }
}
impl Node for ResultConverter {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co| async move {
            let mut gen = self.child.clone().run(world.clone(), entity);
            let node_result = complete_or_yield(&co, &mut gen).await;
            match node_result {
                NodeResult::Aborted => { node_result },
                _ => { (self.convert)(node_result.into()).into() },
            }
        };
        Box::new(Gen::new(producer))
    }
}
