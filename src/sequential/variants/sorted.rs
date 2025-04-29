use std::{cmp::Reverse, sync::Mutex};

use ordered_float::OrderedFloat;

use super::{ScoredSequence, Scorer, result_and, result_forced, result_last, result_or};
use crate as bevior_tree;
use crate::node::prelude::*;

pub mod prelude {
    pub use super::{
        ScoreOrderedForcedSequence, ScoreOrderedSequentialAnd, ScoreOrderedSequentialOr,
        ScoredForcedSelector, pick_max, pick_sorted,
    };
}

/// Sort descending by score.
pub fn pick_sorted(scores: Vec<f32>) -> Vec<usize> {
    let mut enumerated: Vec<(usize, f32)> = scores.into_iter().enumerate().collect();
    enumerated.sort_by_key(|(_, score)| Reverse(OrderedFloat(*score)));
    enumerated.into_iter().map(|(index, _)| index).collect()
}
pub fn pick_max(scores: Vec<f32>) -> Vec<usize> {
    scores
        .into_iter()
        .enumerate()
        .max_by_key(|(_, score)| OrderedFloat(*score))
        .map(|(index, _)| index)
        .into_iter()
        .collect()
}

/// Node that runs children while their result is Success.
/// Children are sorted descending by score on enter the node.
#[delegate_node(delegate)]
pub struct ScoreOrderedSequentialAnd {
    delegate: ScoredSequence,
}
impl ScoreOrderedSequentialAnd {
    pub fn new(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>) -> Self {
        Self {
            delegate: ScoredSequence::new(nodes, pick_sorted, result_and),
        }
    }
}

/// Node that runs children while their result is Failure.
/// Children are sorted descending by score on enter the node.
#[delegate_node(delegate)]
pub struct ScoreOrderedSequentialOr {
    delegate: ScoredSequence,
}
impl ScoreOrderedSequentialOr {
    pub fn new(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>) -> Self {
        Self {
            delegate: ScoredSequence::new(nodes, pick_sorted, result_or),
        }
    }
}

/// Node that runs all children.
/// Children are sorted descending by score on enter the node.
#[delegate_node(delegate)]
pub struct ScoreOrderedForcedSequence {
    delegate: ScoredSequence,
}
impl ScoreOrderedForcedSequence {
    pub fn new(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>) -> Self {
        Self {
            delegate: ScoredSequence::new(nodes, pick_sorted, result_last),
        }
    }
}

/// Node that runs just one child with highest score on enter the node.
#[delegate_node(delegate)]
pub struct ScoredForcedSelector {
    delegate: ScoredSequence,
}
impl ScoredForcedSelector {
    pub fn new(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>) -> Self {
        Self {
            delegate: ScoredSequence::new(nodes, pick_max, result_forced),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tester_util::prelude::*;

    #[test]
    fn test_score_ordered_sequential_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = ScoreOrderedSequentialAnd::new(vec![
            pair_node_scorer_fn(TesterTask::<0>::new(1, NodeResult::Success), |In(_)| 0.1),
            pair_node_scorer_fn(TesterTask::<1>::new(1, NodeResult::Success), |In(_)| 0.3),
            pair_node_scorer_fn(TesterTask::<2>::new(1, NodeResult::Failure), |In(_)| 0.2),
            pair_node_scorer_fn(TesterTask::<3>::new(1, NodeResult::Success), |In(_)| 0.4),
        ]);
        let _entity = app
            .world_mut()
            .spawn(BehaviorTreeBundle::from_root(sequence))
            .id();
        app.update();
        app.update(); // 3
        app.update(); // 1
        app.update(); // 2, sequence complete with Failure
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 3,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 3,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(found == &expected, "Result mismatch. found: {:?}", found);
    }

    #[test]
    fn test_score_ordered_sequential_or() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = ScoreOrderedSequentialOr::new(vec![
            pair_node_scorer_fn(TesterTask::<0>::new(1, NodeResult::Failure), |In(_)| 0.1),
            pair_node_scorer_fn(TesterTask::<1>::new(1, NodeResult::Failure), |In(_)| 0.3),
            pair_node_scorer_fn(TesterTask::<2>::new(1, NodeResult::Success), |In(_)| 0.2),
            pair_node_scorer_fn(TesterTask::<3>::new(1, NodeResult::Failure), |In(_)| 0.4),
        ]);
        let _entity = app
            .world_mut()
            .spawn(BehaviorTreeBundle::from_root(sequence))
            .id();
        app.update();
        app.update(); // 3
        app.update(); // 1
        app.update(); // 2, sequence complete with Success
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 3,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 3,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(found == &expected, "Result mismatch. found: {:?}", found);
    }

    #[test]
    fn test_score_ordered_forced_sequence() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = ScoreOrderedForcedSequence::new(vec![
            pair_node_scorer_fn(TesterTask::<0>::new(1, NodeResult::Failure), |In(_)| 0.1),
            pair_node_scorer_fn(TesterTask::<1>::new(1, NodeResult::Failure), |In(_)| 0.3),
            pair_node_scorer_fn(TesterTask::<2>::new(1, NodeResult::Success), |In(_)| 0.2),
            pair_node_scorer_fn(TesterTask::<3>::new(1, NodeResult::Failure), |In(_)| 0.4),
        ]);
        let _entity = app
            .world_mut()
            .spawn(BehaviorTreeBundle::from_root(sequence))
            .id();
        app.update();
        app.update(); // 3
        app.update(); // 1
        app.update(); // 2
        app.update(); // 0, sequence complete
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 3,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 3,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 0,
                    frame: 4,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(found == &expected, "Result mismatch. found: {:?}", found);
    }

    #[test]
    fn test_score_ordered_forced_selector() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = ScoredForcedSelector::new(vec![
            pair_node_scorer_fn(TesterTask::<0>::new(1, NodeResult::Failure), |In(_)| 0.1),
            pair_node_scorer_fn(TesterTask::<1>::new(1, NodeResult::Failure), |In(_)| 0.3),
            pair_node_scorer_fn(TesterTask::<2>::new(1, NodeResult::Success), |In(_)| 0.2),
            pair_node_scorer_fn(TesterTask::<3>::new(1, NodeResult::Failure), |In(_)| 0.4),
        ]);
        let _entity = app
            .world_mut()
            .spawn(BehaviorTreeBundle::from_root(sequence))
            .id();
        app.update();
        app.update(); // 3, sequence complete
        app.update(); // nop
        let expected = TestLog {
            log: vec![TestLogEntry {
                task_id: 3,
                updated_count: 0,
                frame: 1,
            }],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(found == &expected, "Result mismatch. found: {:?}", found);
    }
}
