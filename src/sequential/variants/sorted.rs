use std::cmp::Reverse;

use bevy::ecs::entity::Entity;
use bevy::ecs::system::{In, IntoSystem};
use ordered_float::OrderedFloat;

use super::{ScoredSequence, ScorerBuilder, Picker, PickerBuilder, AndResultStrategy, ForcedResultStrategy, LastResultStrategy, OrResultStrategy};
use crate as bevior_tree;
use crate::node::prelude::*;

pub mod prelude {
    pub use super::{
        ScoreOrderedForcedSequence, ScoreOrderedSequentialAnd, ScoreOrderedSequentialOr,
        ScoredForcedSelector, MaxPickerBuilder, SortedPickerBuilder,
    };
}

/// Sort descending by score.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SortedPickerBuilder;
#[cfg_attr(feature = "serde", typetag::serde)]
impl PickerBuilder for SortedPickerBuilder {
    fn build(&self) -> Box<dyn Picker> {
        Box::new(IntoSystem::into_system(
            |In((scores, _entity)): In<(Vec<f32>, Entity)>| -> Vec<usize> {
                let mut enumerated: Vec<(usize, f32)> = scores.into_iter().enumerate().collect();
                enumerated.sort_by_key(|(_, score)| Reverse(OrderedFloat(*score)));
                enumerated.into_iter().map(|(index, _)| index).collect()
            }
        ))
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaxPickerBuilder;
#[cfg_attr(feature = "serde", typetag::serde)]
impl PickerBuilder for MaxPickerBuilder {
    fn build(&self) -> Box<dyn Picker> {
        Box::new(IntoSystem::into_system(
            |In((scores, _entity)): In<(Vec<f32>, Entity)>| -> Vec<usize> {
                scores
                    .into_iter()
                    .enumerate()
                    .max_by_key(|(_, score)| OrderedFloat(*score))
                    .map(|(index, _)| index)
                    .into_iter()
                    .collect()
            }
        ))
    }
}


/// Node that runs children while their result is Success.
/// Children are sorted descending by score on enter the node.
#[delegate_node(delegate)]
pub struct ScoreOrderedSequentialAnd {
    delegate: ScoredSequence,
}
impl ScoreOrderedSequentialAnd {
    pub fn new(nodes: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>) -> Self {
        Self {
            delegate: ScoredSequence::new(nodes, SortedPickerBuilder, AndResultStrategy),
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
    pub fn new(nodes: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>) -> Self {
        Self {
            delegate: ScoredSequence::new(nodes, SortedPickerBuilder, OrResultStrategy),
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
    pub fn new(nodes: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>) -> Self {
        Self {
            delegate: ScoredSequence::new(nodes, SortedPickerBuilder, LastResultStrategy),
        }
    }
}

/// Node that runs just one child with highest score on enter the node.
#[delegate_node(delegate)]
pub struct ScoredForcedSelector {
    delegate: ScoredSequence,
}
impl ScoredForcedSelector {
    pub fn new(nodes: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>) -> Self {
        Self {
            delegate: ScoredSequence::new(nodes, MaxPickerBuilder, ForcedResultStrategy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ConstantScorerBuilder;
    use crate::tester_util::prelude::*;

    #[test]
    fn test_score_ordered_sequential_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = ScoreOrderedSequentialAnd::new(vec![
            (Box::new(TesterTask0::new(1, NodeResult::Success)), Box::new(ConstantScorerBuilder { score: 0.1 })),
            (Box::new(TesterTask1::new(1, NodeResult::Success)), Box::new(ConstantScorerBuilder { score: 0.3 })),
            (Box::new(TesterTask2::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.2 })),
            (Box::new(TesterTask3::new(1, NodeResult::Success)), Box::new(ConstantScorerBuilder { score: 0.4 })),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
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
            (Box::new(TesterTask0::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.1 })),
            (Box::new(TesterTask1::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.3 })),
            (Box::new(TesterTask2::new(1, NodeResult::Success)), Box::new(ConstantScorerBuilder { score: 0.2 })),
            (Box::new(TesterTask3::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.4 })),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
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
            (Box::new(TesterTask0::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.1 })),
            (Box::new(TesterTask1::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.3 })),
            (Box::new(TesterTask2::new(1, NodeResult::Success)), Box::new(ConstantScorerBuilder { score: 0.2 })),
            (Box::new(TesterTask3::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.4 })),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
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
            (Box::new(TesterTask0::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.1 })),
            (Box::new(TesterTask1::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.3 })),
            (Box::new(TesterTask2::new(1, NodeResult::Success)), Box::new(ConstantScorerBuilder { score: 0.2 })),
            (Box::new(TesterTask3::new(1, NodeResult::Failure)), Box::new(ConstantScorerBuilder { score: 0.4 })),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
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
