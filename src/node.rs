//! Abstract representation of node of behavior tree.

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


/// State of pending, work in progress nodes.
/// `#[derive(NodeState)]` is available.
pub trait NodeState: 'static + Send + Sync {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}


/// Result of completed nodes.
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

/// Status of execution of the node.
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


/// Node of behavior trees.
/// Nodes should not hold the state of execution.
/// Nodes take state of execution as argument, do things with it, then return the status of the execution.
pub trait Node: 'static + Send + Sync {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus;
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus;
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>);
}


/// Trait to pair the node and the state.
/// Also `#[with_state(State)]` is available for simple cases.
/// 
/// Nodes take `state: Box<dyn NodeState>`, so this trait help downcast it.
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


/// Shorthand to delegate node.
/// Also `#[delegate_node(target)]` is available for simple cases.
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
