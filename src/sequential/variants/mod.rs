use std::sync::Mutex;

use bevy::ecs::{
    entity::Entity,
    system::{In, IntoSystem},
};

use super::{ScoredSequence, Scorer};
use crate as bevior_tree;
use crate::node::prelude::*;

pub mod sorted;

#[cfg(feature = "random")]
pub mod random;

pub mod prelude {
    pub use super::{
        ForcedSequence, Selector, Sequence, SequentialAnd, SequentialOr, pick_identity,
        random::prelude::*, score_uniform, sorted::prelude::*,
    };
}

pub fn score_uniform(nodes: Vec<Box<dyn Node>>) -> Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)> {
    fn score(_: In<Entity>) -> f32 {
        1.0
    }
    nodes
        .into_iter()
        .map(|node| {
            let scorer: Box<dyn Scorer> = Box::new(IntoSystem::into_system(score));
            (node, Mutex::new(scorer))
        })
        .collect()
}

pub fn pick_identity(scores: Vec<f32>) -> Vec<usize> {
    (0..scores.len()).collect()
}

pub fn result_and(results: Vec<Option<NodeResult>>) -> Option<NodeResult> {
    if results.contains(&Some(NodeResult::Failure)) {
        Some(NodeResult::Failure)
    } else if results.contains(&None) {
        None
    } else {
        Some(NodeResult::Success)
    }
}
pub fn result_or(results: Vec<Option<NodeResult>>) -> Option<NodeResult> {
    if results.contains(&Some(NodeResult::Success)) {
        Some(NodeResult::Success)
    } else if results.contains(&None) {
        None
    } else {
        Some(NodeResult::Failure)
    }
}
pub fn result_last(results: Vec<Option<NodeResult>>) -> Option<NodeResult> {
    if results.contains(&None) {
        None
    } else {
        match results.last() {
            Some(result) => *result,
            None => Some(NodeResult::Failure),
        }
    }
}
pub fn result_forced(results: Vec<Option<NodeResult>>) -> Option<NodeResult> {
    results.into_iter().find_map(|r| r)
}

pub type Sequence = SequentialAnd;
/// Node that runs children in order while their result is Success.
#[delegate_node(delegate)]
pub struct SequentialAnd {
    delegate: ScoredSequence,
}
impl SequentialAnd {
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            delegate: ScoredSequence::new(score_uniform(nodes), pick_identity, result_and),
        }
    }
}

pub type Selector = SequentialOr;
/// Node that runs children in order until one of them returns Success.
#[delegate_node(delegate)]
pub struct SequentialOr {
    delegate: ScoredSequence,
}
impl SequentialOr {
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            delegate: ScoredSequence::new(score_uniform(nodes), pick_identity, result_or),
        }
    }
}

/// Node that runs all children in order.
#[delegate_node(delegate)]
pub struct ForcedSequence {
    delegate: ScoredSequence,
}
impl ForcedSequence {
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            delegate: ScoredSequence::new(score_uniform(nodes), pick_identity, result_last),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tester_util::prelude::*;

    #[test]
    fn test_sequential_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = Sequence::new(vec![
            Box::new(TesterTask::<0>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<1>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<2>::new(1, NodeResult::Failure)),
            Box::new(TesterTask::<3>::new(1, NodeResult::Success)),
        ]);
        let _entity = app
            .world_mut()
            .spawn(BehaviorTree::new(sequence))
            .id();
        app.update();
        app.update(); // 0
        app.update(); // 1
        app.update(); // 2, sequence complete with Failure
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 0,
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
        assert!(
            found == &expected,
            "SequentialAnd should match result. found: {:?}",
            found
        );
    }

    #[test]
    fn test_sequential_or() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = Selector::new(vec![
            Box::new(TesterTask::<0>::new(1, NodeResult::Failure)),
            Box::new(TesterTask::<1>::new(1, NodeResult::Failure)),
            Box::new(TesterTask::<2>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<3>::new(1, NodeResult::Failure)),
        ]);
        let _entity = app
            .world_mut()
            .spawn(BehaviorTree::new(sequence))
            .id();
        app.update();
        app.update(); // 0
        app.update(); // 1
        app.update(); // 2, sequence complete with Success
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 0,
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
        assert!(
            found == &expected,
            "SequentialOr should match result. found: {:?}",
            found
        );
    }

    #[test]
    fn test_forced_sequence() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = ForcedSequence::new(vec![
            Box::new(TesterTask::<0>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<1>::new(1, NodeResult::Failure)),
            Box::new(TesterTask::<2>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<3>::new(1, NodeResult::Failure)),
        ]);
        let _entity = app
            .world_mut()
            .spawn(BehaviorTree::new(sequence))
            .id();
        app.update();
        app.update(); // 0
        app.update(); // 1
        app.update(); // 2,
        app.update(); // 3, sequence complete
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 0,
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
                    task_id: 3,
                    updated_count: 0,
                    frame: 4,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "ForcedSequence should run all the tasks. found: {:?}",
            found
        );
    }
}
