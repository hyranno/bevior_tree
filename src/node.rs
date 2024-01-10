

pub trait NodeState: 'static + Send + Sync{}

pub enum NodeResult {
    Success,
    Failure,
}

pub enum NodeStatus {
    Beginning,
    Pending(Box<dyn NodeState>),
    Complete(NodeResult),
}

pub trait Node: 'static + Send + Sync {
    fn begin(&self) -> NodeStatus;
    fn resume(&self, state: &Box<dyn NodeState>) -> NodeStatus;
}

