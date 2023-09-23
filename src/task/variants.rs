use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use crate::{Node, NodeGen, NodeResult, nullable_access::NullableWorldAccess};

use super::{TaskChecker, TaskState};


/// TaskChecker which returns constant.
pub struct ConstantChecker {
    pub result: TaskState,
}
impl TaskChecker for ConstantChecker {
    type Param<'w, 's> = ();
    fn check (
        &self,
        _entity: bevy::prelude::Entity,
        _param: <<Self as TaskChecker>::Param<'_, '_> as bevy::ecs::system::SystemParam>::Item<'_, '_>,
    ) -> TaskState {
        self.result
    }
}

/// Task that immediately returns specified value.
pub struct QuickReturn {
    pub result: NodeResult,
}
impl Node for QuickReturn {
    fn run(self: Arc<Self>, _world: Arc<Mutex<NullableWorldAccess>>, _entity: bevy::prelude::Entity) -> Box<dyn NodeGen> {
        let producer = |_| async move {self.result};
        Box::new(Gen::new(producer))
    }
}

