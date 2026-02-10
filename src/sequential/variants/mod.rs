use bevy::ecs::{
    entity::Entity,
    system::{In, IntoSystem},
};

use super::{Picker, PickerBuilder, ResultStrategy, ScoredSequence, Scorer, ScorerBuilder};
use crate as bevior_tree;
use crate::node::prelude::*;

pub mod sorted;

#[cfg(feature = "random")]
pub mod random;

pub mod prelude {
    pub use super::{
        ForcedSequence, IdentityPickerBuilder, Selector, Sequence, SequentialAnd, SequentialOr,
        random::prelude::*, score_uniform, sorted::prelude::*,
    };
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UniformScorerBuilder;
#[cfg_attr(feature = "serde", typetag::serde)]
impl ScorerBuilder for UniformScorerBuilder {
    fn build(&self) -> Box<dyn Scorer> {
        Box::new(IntoSystem::into_system(|_: In<Entity>| 1.0f32))
    }
}

pub fn score_uniform(nodes: Vec<Box<dyn Node>>) -> Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)> {
    nodes
        .into_iter()
        .map(|node| {
            (
                node,
                Box::new(UniformScorerBuilder) as Box<dyn ScorerBuilder>,
            )
        })
        .collect()
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct ConstantScorerBuilder {
    score: f32,
}
#[cfg_attr(feature = "serde", typetag::serde)]
impl ScorerBuilder for ConstantScorerBuilder {
    fn build(&self) -> Box<dyn Scorer> {
        let score = self.score;
        Box::new(IntoSystem::into_system(
            move |In(_entity): In<Entity>| -> f32 { score },
        ))
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IdentityPickerBuilder;
#[cfg_attr(feature = "serde", typetag::serde)]
impl PickerBuilder for IdentityPickerBuilder {
    fn build(&self) -> Box<dyn Picker> {
        Box::new(IntoSystem::into_system(
            |In((scores, _entity)): In<(Vec<f32>, Entity)>| -> Vec<usize> {
                let count = scores.len();
                (0..count).collect()
            },
        ))
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AndResultStrategy;
#[cfg_attr(feature = "serde", typetag::serde)]
impl ResultStrategy for AndResultStrategy {
    fn construct(&self, results: Vec<Option<NodeResult>>) -> Option<NodeResult> {
        if results.contains(&Some(NodeResult::Failure)) {
            Some(NodeResult::Failure)
        } else if results.contains(&None) {
            None
        } else {
            Some(NodeResult::Success)
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrResultStrategy;
#[cfg_attr(feature = "serde", typetag::serde)]
impl ResultStrategy for OrResultStrategy {
    fn construct(&self, results: Vec<Option<NodeResult>>) -> Option<NodeResult> {
        if results.contains(&Some(NodeResult::Success)) {
            Some(NodeResult::Success)
        } else if results.contains(&None) {
            None
        } else {
            Some(NodeResult::Failure)
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LastResultStrategy;
#[cfg_attr(feature = "serde", typetag::serde)]
impl ResultStrategy for LastResultStrategy {
    fn construct(&self, results: Vec<Option<NodeResult>>) -> Option<NodeResult> {
        if results.contains(&None) {
            None
        } else {
            match results.last() {
                Some(result) => *result,
                None => Some(NodeResult::Failure),
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ForcedResultStrategy;
#[cfg_attr(feature = "serde", typetag::serde)]
impl ResultStrategy for ForcedResultStrategy {
    fn construct(&self, results: Vec<Option<NodeResult>>) -> Option<NodeResult> {
        results.into_iter().find_map(|r| r)
    }
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
            delegate: ScoredSequence::new(
                score_uniform(nodes),
                IdentityPickerBuilder,
                AndResultStrategy,
            ),
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
            delegate: ScoredSequence::new(
                score_uniform(nodes),
                IdentityPickerBuilder,
                OrResultStrategy,
            ),
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
            delegate: ScoredSequence::new(
                score_uniform(nodes),
                IdentityPickerBuilder,
                LastResultStrategy,
            ),
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
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let sequence = Sequence::new(vec![
            Box::new(TesterTask0::new(1, NodeResult::Success)),
            Box::new(TesterTask1::new(1, NodeResult::Success)),
            Box::new(TesterTask2::new(1, NodeResult::Failure)),
            Box::new(TesterTask3::new(1, NodeResult::Success)),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
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
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let sequence = Selector::new(vec![
            Box::new(TesterTask0::new(1, NodeResult::Failure)),
            Box::new(TesterTask1::new(1, NodeResult::Failure)),
            Box::new(TesterTask2::new(1, NodeResult::Success)),
            Box::new(TesterTask3::new(1, NodeResult::Failure)),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
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
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let sequence = ForcedSequence::new(vec![
            Box::new(TesterTask0::new(1, NodeResult::Success)),
            Box::new(TesterTask1::new(1, NodeResult::Failure)),
            Box::new(TesterTask2::new(1, NodeResult::Success)),
            Box::new(TesterTask3::new(1, NodeResult::Failure)),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
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
