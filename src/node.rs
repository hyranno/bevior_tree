
use std::{any::Any, ops::Not};
use bevy::prelude::{World, Entity};

pub mod prelude {
    pub use super::{
        Node, NodeStatus, NodeResult, NodeState,
        WithState, NodeStateError,
    };
    pub use derive_nodestate::NodeState;
    pub use macro_withstate::with_state;
    pub use macro_delegatenode::delegate_node;
}


pub trait NodeState: 'static + Send + Sync {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeResult {
    Success,
    Failure,
}
impl Not for NodeResult {
    type Output = NodeResult;
    fn not(self) -> Self::Output {
        match self {
            NodeResult::Success => NodeResult::Failure,
            NodeResult::Failure => NodeResult::Success,
        }
    }
}

pub enum NodeStatus {
    Beginning,
    Pending(Box<dyn NodeState>),
    Complete(NodeResult),
}
impl NodeStatus {
    pub fn result(&self) -> Option<NodeResult> {
        match self {
            &NodeStatus::Complete(result) => Some(result),
            _ => None
        }
    }
}

pub trait Node: 'static + Send + Sync {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus;
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus;
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>);
}


pub trait WithState<State: NodeState>: Node {
    fn downcast(state: Box<dyn NodeState>) -> Result<State, NodeStateError> {
        let result = state.into_any().downcast::<State>();
        match result {
            Ok(state) => Ok(*state),
            Err(_) => Err(NodeStateError::InvalidTypeOfState),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq,)]
pub enum NodeStateError {
    InvalidTypeOfState,
}


pub trait DelegateNode: 'static + Send + Sync {
    fn delegate_node(&self) -> &dyn Node;
}
impl<T: DelegateNode> Node for T {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate_node().begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate_node().resume(world, entity, state)
    }
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        self.delegate_node().force_exit(world, entity, state)
    }
}
