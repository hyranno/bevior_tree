
use crate as bevior_tree;
use crate::node::prelude::*;
use super::ResultConverter;


pub mod prelude {
    pub use super::{
        Invert, ForceResult,
    };
}


/// Invert the result of the child.
#[delegate_node(delegate)]
pub struct Invert {
    delegate: ResultConverter,
}
impl Invert {
    pub fn new(child: impl Node) -> Self {
        Self {
            delegate: ResultConverter::new(child, |res| !res)
        }
    }
}

/// Returns the specified result whatever the child returns.
#[delegate_node(delegate)]
pub struct ForceResult {
    delegate: ResultConverter,
}
impl ForceResult {
    pub fn new(child: impl Node, result: NodeResult) -> Self {
        Self {
            delegate: ResultConverter::new(child, move |_| result)
        }
    }
}



#[cfg(test)]
mod tests {
    use crate::tester_util::prelude::*;

    #[test]
    fn test_invert() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let converter = Invert::new(task);
        let entity = app.world_mut().spawn(BehaviorTreeBundle::from_root(converter)).id();
        app.update();
        app.update();
        let status = app.world().get::<TreeStatus>(entity);
        assert!(
            if let Some(TreeStatus(NodeStatus::Complete(result))) = status {
                result == &NodeResult::Failure
            } else {false},
            "Invert should match the result."
        );
    }

    #[test]
    fn test_force_result() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let converter = ForceResult::new(task, NodeResult::Failure);
        let entity = app.world_mut().spawn(BehaviorTreeBundle::from_root(converter)).id();
        app.update();
        app.update();
        let status = app.world().get::<TreeStatus>(entity);
        assert!(
            if let Some(TreeStatus(NodeStatus::Complete(result))) = status {
                result == &NodeResult::Failure
            } else {false},
            "ForceResult should match the result."
        );
    }

}

