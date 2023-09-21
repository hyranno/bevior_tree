use std::sync::{Arc, Mutex};
use bevy::prelude::*;

use crate::{Node, NodeGen, NodeResult};
use crate::nullable_access::NullableWorldAccess;
use super::{ScoredSequence, NodeScorer, NodeScorerImpl, ConstantScorer};

pub mod sorted;

#[cfg(feature = "random")]
pub mod random;

pub fn score_uniform(nodes: Vec<Arc<dyn Node>>) -> Vec<Box<dyn NodeScorer>> {
    nodes.iter().map(|node| Box::new(
        NodeScorerImpl::new(ConstantScorer {score: 1.0}, node.clone())
    ) as Box<dyn NodeScorer>).collect()
}

pub fn pick_identity(nodes: Vec<(f32, Arc<dyn Node>)>) -> Vec<(f32, Arc<dyn Node>)> {
    nodes
}


pub type Sequence = SequentialAnd;
/// Node that runs children in order while their result is Success.
pub struct SequentialAnd {
    delegate: Arc<ScoredSequence>,
}
impl SequentialAnd {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: ScoredSequence::new(
            score_uniform(nodes),
            pick_identity,
            |res| res==NodeResult::Success,
            NodeResult::Success,
        )})
    }
}
impl Node for SequentialAnd {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}

pub type Selector = SequentialOr;
/// Node that runs children in order until one of them returns Success.
pub struct SequentialOr {
    delegate: Arc<ScoredSequence>,
}
impl SequentialOr {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: ScoredSequence::new(
            score_uniform(nodes),
            pick_identity,
            |res| res==NodeResult::Failure,
            NodeResult::Failure,
        )})
    }
}
impl Node for SequentialOr {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}

/// Node that runs all children in order.
pub struct ForcedSequence {
    delegate: Arc<ScoredSequence>,
}
impl ForcedSequence {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: ScoredSequence::new(
            score_uniform(nodes),
            pick_identity,
            |_| true,
            NodeResult::Success,
        )})
    }
}
impl Node for ForcedSequence {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}



#[cfg(test)]
mod tests {
    use crate::*;
    use crate::task::*;
    use crate::tester_util::{TesterPlugin, TesterTask, TestLog, TestLogEntry};
    use super::*;

    #[test]
    fn test_sequential_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task0 = TesterTask::<0>::new(1, TaskState::Success);
        let task1 = TesterTask::<1>::new(1, TaskState::Success);
        let task2 = TesterTask::<2>::new(1, TaskState::Failure);
        let task3 = TesterTask::<3>::new(1, TaskState::Success);
        let sequence = Sequence::new(vec![task0, task1, task2, task3]);
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 0
        app.update();  // 1
        app.update();  // 2, sequence complete with Failure
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 3},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "SequentialAnd should match result."
        );
    }

    #[test]
    fn test_sequential_or() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task0 = TesterTask::<0>::new(1, TaskState::Failure);
        let task1 = TesterTask::<1>::new(1, TaskState::Failure);
        let task2 = TesterTask::<2>::new(1, TaskState::Success);
        let task3 = TesterTask::<3>::new(1, TaskState::Failure);
        let sequence = Selector::new(vec![task0, task1, task2, task3]);
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 0
        app.update();  // 1
        app.update();  // 2, sequence complete with Success
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 3},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "SequentialOr should match result."
        );
    }

    #[test]
    fn test_forced_sequence() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task0 = TesterTask::<0>::new(1, TaskState::Success);
        let task1 = TesterTask::<1>::new(1, TaskState::Failure);
        let task2 = TesterTask::<2>::new(1, TaskState::Success);
        let task3 = TesterTask::<3>::new(1, TaskState::Failure);
        let sequence = ForcedSequence::new(vec![task0, task1, task2, task3]);
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 0
        app.update();  // 1
        app.update();  // 2,
        app.update();  // 3, sequence complete
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 3},
            TestLogEntry {task_id: 3, updated_count: 0, frame: 4},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "ForcedSequence should run all the tasks."
        );
    }

}