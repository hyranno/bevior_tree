//! Sequencial composit nodes.

use std::sync::{Arc, Mutex};
use genawaiter::sync::Gen;

use bevy::ecs::entity::Entity;

use super::{Node, NodeGen, NodeResult, complete_or_yield, nullable_access::NullableWorldAccess};


pub struct SequenceWhile {
    nodes: Vec<Arc<dyn Node>>,
    cond: Box<dyn Fn(NodeResult) -> bool + 'static + Send + Sync>,
    complete_value: NodeResult,
}
impl SequenceWhile {
    pub fn new(
        nodes: Vec<Arc<dyn Node>>,
        cond: impl Fn(NodeResult)->bool + 'static + Send + Sync,
        complete_value: NodeResult
    ) -> Arc<Self> {
        Arc::new(Self { nodes, cond: Box::new(cond), complete_value })
    }
}
impl Node for SequenceWhile {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        let producer = |co| async move {
            for node in self.nodes.iter() {
                let mut gen = node.clone().run(world.clone(), entity);
                let node_result = complete_or_yield(&co, &mut gen).await;
                if !(self.cond)(node_result) {
                    return node_result;
                }
            }
            self.complete_value
        };
        Box::new(Gen::new(producer))
    }
}

pub type Sequence = SequencialAnd;
pub struct SequencialAnd {
    delegate: Arc<SequenceWhile>,
}
impl SequencialAnd {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: SequenceWhile::new(
            nodes, |res| res==NodeResult::Success, NodeResult::Success
        )})
    }
}
impl Node for SequencialAnd {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}

pub type Selector = SequencialOr;
pub struct SequencialOr {
    delegate: Arc<SequenceWhile>,
}
impl SequencialOr {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: SequenceWhile::new(
            nodes, |res| res==NodeResult::Failure, NodeResult::Failure
        )})
    }
}
impl Node for SequencialOr {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}

pub struct ForcedSequence {
    delegate: Arc<SequenceWhile>,
}
impl ForcedSequence {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: SequenceWhile::new(
            nodes, |_| true, NodeResult::Success
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
    use crate::tester_util::*;

    #[test]
    fn test_sequencial_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task0 = TesterTask::new(0, 1, TaskState::Success);
        let task1 = TesterTask::new(1, 1, TaskState::Success);
        let task2 = TesterTask::new(2, 1, TaskState::Failure);
        let task3 = TesterTask::new(3, 1, TaskState::Success);
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
            "SequencialAnd should match result."
        );
    }

    #[test]
    fn test_sequencial_or() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task0 = TesterTask::new(0, 1, TaskState::Failure);
        let task1 = TesterTask::new(1, 1, TaskState::Failure);
        let task2 = TesterTask::new(2, 1, TaskState::Success);
        let task3 = TesterTask::new(3, 1, TaskState::Failure);
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
            "SequencialOr should match result."
        );
    }

    #[test]
    fn test_forced_sequence() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task0 = TesterTask::new(0, 1, TaskState::Success);
        let task1 = TesterTask::new(1, 1, TaskState::Failure);
        let task2 = TesterTask::new(2, 1, TaskState::Success);
        let task3 = TesterTask::new(3, 1, TaskState::Failure);
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