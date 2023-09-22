use std::{cmp::Reverse, sync::{Arc, Mutex}};
use bevy::prelude::*;
use ordered_float::OrderedFloat;

use crate::{Node, NodeGen, NodeResult};
use crate::nullable_access::NullableWorldAccess;
use crate::sequential::{ScoredSequence, NodeScorer};
use super::last_result;


/// Sort descending by score.
pub fn pick_sorted(mut nodes: Vec<(f32, Arc<dyn Node>)>) -> Vec<(f32, Arc<dyn Node>)> {
    nodes.sort_by_key(|(score, _)| Reverse(OrderedFloat(*score)));
    nodes
}
pub fn pick_max(nodes: Vec<(f32, Arc<dyn Node>)>) -> Vec<(f32, Arc<dyn Node>)> {
    let picked = nodes.into_iter().max_by_key(|(score, _)| OrderedFloat(*score));
    match picked {
        Some(node) => vec![node],
        None => vec![],
    }
}

/// Node that runs children while their result is Success.
/// Children are sorted descending by score on enter the node.
pub struct ScoreOrderedSequentialAnd {
    delegate: Arc<ScoredSequence>,
}
impl Node for ScoreOrderedSequentialAnd {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl ScoreOrderedSequentialAnd {
    pub fn new(node_scorers: Vec<Box<dyn NodeScorer>>) -> Arc<Self> {
        Arc::new(Self {delegate: ScoredSequence::new(
            node_scorers,
            pick_sorted,
            |res| res==NodeResult::Success,
            |_| NodeResult::Success,
        )})
    }
}

/// Node that runs children while their result is Failure.
/// Children are sorted descending by score on enter the node.
pub struct ScoreOrderedSequentialOr {
    delegate: Arc<ScoredSequence>,
}
impl Node for ScoreOrderedSequentialOr {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl ScoreOrderedSequentialOr {
    pub fn new(node_scorers: Vec<Box<dyn NodeScorer>>) -> Arc<Self> {
        Arc::new(Self {delegate: ScoredSequence::new(
            node_scorers,
            pick_sorted,
            |res| res==NodeResult::Failure,
            last_result,
        )})
    }
}

/// Node that runs all children.
/// Children are sorted descending by score on enter the node.
pub struct ScoreOrderedForcedSequence {
    delegate: Arc<ScoredSequence>,
}
impl Node for ScoreOrderedForcedSequence {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl ScoreOrderedForcedSequence {
    pub fn new(node_scorers: Vec<Box<dyn NodeScorer>>) -> Arc<Self> {
        Arc::new(Self {delegate: ScoredSequence::new(
            node_scorers,
            pick_sorted,
            |_| true,
            last_result,
        )})
    }
}

/// Node that runs just one child with highest score on enter the node.
pub struct ScoredForcedSelector {
    delegate: Arc<ScoredSequence>,
}
impl Node for ScoredForcedSelector {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl ScoredForcedSelector {
    pub fn new(node_scorers: Vec<Box<dyn NodeScorer>>) -> Arc<Self> {
        Arc::new(Self {delegate: ScoredSequence::new(
            node_scorers,
            pick_max,
            |_| false,
            |_| NodeResult::Failure,  // Be used only when the nodes is empty.
        )})
    }
}


#[cfg(test)]
mod tests {
    use crate::*;
    use crate::task::*;
    use crate::tester_util::{TesterPlugin, TesterTask, TestLog, TestLogEntry};
    use crate::sequential::{NodeScorerImpl, variants::ConstantScorer};
    use super::*;

    #[test]
    fn test_score_ordered_sequential_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let sequence = ScoreOrderedSequentialAnd::new(vec![
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.1},
                TesterTask::<0>::new(1, TaskState::Success)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.3},
                TesterTask::<1>::new(1, TaskState::Success)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.2},
                TesterTask::<2>::new(1, TaskState::Failure)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.4},
                TesterTask::<3>::new(1, TaskState::Success)
            )),
        ]);
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 3
        app.update();  // 1
        app.update();  // 2, sequence complete with Failure
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 3},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "ScoreOrderedSequentialAnd should match result."
        );
    }

    #[test]
    fn test_score_ordered_sequential_or() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let sequence = ScoreOrderedSequentialOr::new(vec![
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.1},
                TesterTask::<0>::new(1, TaskState::Failure)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.3},
                TesterTask::<1>::new(1, TaskState::Failure)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.2},
                TesterTask::<2>::new(1, TaskState::Success)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.4},
                TesterTask::<3>::new(1, TaskState::Failure)
            )),
        ]);
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 3
        app.update();  // 1
        app.update();  // 2, sequence complete with Success
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 3},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "ScoreOrderedSequentialAnd should match result."
        );
    }

    #[test]
    fn test_score_ordered_forced_sequence() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let sequence = ScoreOrderedForcedSequence::new(vec![
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.1},
                TesterTask::<0>::new(1, TaskState::Failure)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.3},
                TesterTask::<1>::new(1, TaskState::Failure)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.2},
                TesterTask::<2>::new(1, TaskState::Success)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.4},
                TesterTask::<3>::new(1, TaskState::Failure)
            )),
        ]);
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 3
        app.update();  // 1
        app.update();  // 2
        app.update();  // 0, sequence complete
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 3},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 4},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "ScoreOrderedForcedSequence should match result."
        );
    }

    #[test]
    fn test_score_ordered_forced_selector() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let sequence = ScoredForcedSelector::new(vec![
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.1},
                TesterTask::<0>::new(1, TaskState::Failure)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.3},
                TesterTask::<1>::new(1, TaskState::Failure)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.2},
                TesterTask::<2>::new(1, TaskState::Success)
            )),
            Box::new(NodeScorerImpl::new(
                ConstantScorer {score: 0.4},
                TesterTask::<3>::new(1, TaskState::Failure)
            )),
        ]);
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 3, sequence complete
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "ScoreOrderedForcedSelector should match result."
        );
    }
}