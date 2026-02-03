use std::{marker::PhantomData, sync::Mutex};

use bevy::{
    ecs::system::{In, IntoSystem},
    prelude::{Entity, ResMut, Resource},
};

use rand::{Rng, distr::Uniform, prelude::Distribution};

use super::sorted::{pick_max, pick_sorted};
use super::{ScoredSequence, Scorer, result_and, result_forced, result_last, result_or};
use crate as bevior_tree;
use crate::node::prelude::*;

pub mod prelude {
    pub use super::{
        RandomForcedSelector, RandomOrderedForcedSequence, RandomOrderedSequentialAnd,
        RandomOrderedSequentialOr, randomize_picker,
    };
}

/// Weighted random sampling.
/// Probability of being picked next is proportional to the score.
/// Using algorithm called A-ES by Efraimidis and Spirakis.
pub fn randomize_picker<R, Marker>(
    In((scores, entity)): In<(Vec<f32>, Entity)>,
    mut rng_res: ResMut<RngResource<R, Marker>>,
) -> (Vec<f32>, Entity)
where
    R: Rng + 'static + Send + Sync,
    Marker: 'static + Send + Sync,
{
    let dist = Uniform::<f32>::new(0.0, 1.0).expect("Failed to init uniform distribution.");
    let scores = scores
        .into_iter()
        .map(|score| dist.sample(&mut rng_res.rng).powf(1.0 / score))
        .collect();
    (scores, entity)
}

/// Resource that holds RNG instance.
/// Insert this resource to use random-based nodes.
#[derive(Resource)]
pub struct RngResource<R, Marker>
where
    R: Rng + 'static + Send + Sync,
    Marker: 'static + Send + Sync,
{
    phantom: PhantomData<Marker>,
    rng: R,
}
impl<R, Marker> RngResource<R, Marker>
where
    R: Rng + 'static + Send + Sync,
    Marker: 'static + Send + Sync,
{
    pub fn new(rng: R) -> Self {
        Self {
            phantom: PhantomData,
            rng,
        }
    }
}

/// Node that runs children while their result is Success.
/// Children are sorted random weighted by score on enter the node.
#[delegate_node(delegate)]
pub struct RandomOrderedSequentialAnd {
    delegate: ScoredSequence,
}
impl RandomOrderedSequentialAnd {
    pub fn new<R, Marker>(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>) -> Self
    where
        R: Rng + 'static + Send + Sync,
        Marker: 'static + Send + Sync,
    {
        Self {
            delegate: ScoredSequence::new(
                nodes,
                randomize_picker::<R, Marker>.pipe(pick_sorted),
                result_and,
            ),
        }
    }
}

/// Node that runs children while their result is Failure.
/// Children are sorted random weighted by score on enter the node.
#[delegate_node(delegate)]
pub struct RandomOrderedSequentialOr {
    delegate: ScoredSequence,
}
impl RandomOrderedSequentialOr {
    pub fn new<R, Marker>(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>) -> Self
    where
        R: Rng + 'static + Send + Sync,
        Marker: 'static + Send + Sync,
    {
        Self {
            delegate: ScoredSequence::new(
                nodes,
                randomize_picker::<R, Marker>.pipe(pick_sorted),
                result_or,
            ),
        }
    }
}

/// Node that runs all children.
/// Children are sorted random weighted by score on enter the node.
#[delegate_node(delegate)]
pub struct RandomOrderedForcedSequence {
    delegate: ScoredSequence,
}
impl RandomOrderedForcedSequence {
    pub fn new<R, Marker>(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>) -> Self
    where
        R: Rng + 'static + Send + Sync,
        Marker: 'static + Send + Sync,
    {
        Self {
            delegate: ScoredSequence::new(
                nodes,
                randomize_picker::<R, Marker>.pipe(pick_sorted),
                result_last,
            ),
        }
    }
}

/// Node that runs just one child picked with score-weighted random on enter the node.
#[delegate_node(delegate)]
pub struct RandomForcedSelector {
    delegate: ScoredSequence,
}
impl RandomForcedSelector {
    pub fn new<R, Marker>(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>) -> Self
    where
        R: Rng + 'static + Send + Sync,
        Marker: 'static + Send + Sync,
    {
        Self {
            delegate: ScoredSequence::new(
                nodes,
                randomize_picker::<R, Marker>.pipe(pick_max),
                result_forced,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tester_util::prelude::*;

    use rand::SeedableRng;

    struct RngMarker;

    #[test]
    fn test_random_ordered_sequential_and() {
        let mut app = App::new();
        let rng_res = RngResource::<_, RngMarker>::new(rand::rngs::StdRng::seed_from_u64(224));
        app.insert_resource(rng_res);
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomOrderedSequentialAnd::new::<rand::rngs::StdRng, RngMarker>(vec![
            pair_node_scorer_fn(TesterTask0::new(1, NodeResult::Success), |In(_)| 0.1),
            pair_node_scorer_fn(TesterTask1::new(1, NodeResult::Success), |In(_)| 0.3),
            pair_node_scorer_fn(TesterTask2::new(1, NodeResult::Success), |In(_)| 0.2),
            pair_node_scorer_fn(TesterTask3::new(1, NodeResult::Failure), |In(_)| 0.4),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 1
        app.update(); // 2
        app.update(); // 3, sequence complete with Failure
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 3,
                    updated_count: 0,
                    frame: 3,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomOrderedSequentialAnd should match result. found: {:?}",
            found
        );
    }

    #[test]
    fn test_random_ordered_sequential_or() {
        let mut app = App::new();
        let rng_res = RngResource::<_, RngMarker>::new(rand::rngs::StdRng::seed_from_u64(224));
        app.insert_resource(rng_res);
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomOrderedSequentialOr::new::<rand::rngs::StdRng, RngMarker>(vec![
            pair_node_scorer_fn(TesterTask0::new(1, NodeResult::Failure), |In(_)| 0.1),
            pair_node_scorer_fn(TesterTask1::new(1, NodeResult::Failure), |In(_)| 0.3),
            pair_node_scorer_fn(TesterTask2::new(1, NodeResult::Failure), |In(_)| 0.2),
            pair_node_scorer_fn(TesterTask3::new(1, NodeResult::Success), |In(_)| 0.4),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 1
        app.update(); // 2
        app.update(); // 3, sequence complete with Success
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 3,
                    updated_count: 0,
                    frame: 3,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomOrderedSequentialOr should match result. found: {:?}",
            found
        );
    }

    #[test]
    fn test_random_ordered_forced_sequence() {
        let mut app = App::new();
        let rng_res = RngResource::<_, RngMarker>::new(rand::rngs::StdRng::seed_from_u64(224));
        app.insert_resource(rng_res);
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomOrderedForcedSequence::new::<rand::rngs::StdRng, RngMarker>(vec![
            pair_node_scorer_fn(TesterTask0::new(1, NodeResult::Failure), |In(_)| 0.1),
            pair_node_scorer_fn(TesterTask1::new(1, NodeResult::Failure), |In(_)| 0.3),
            pair_node_scorer_fn(TesterTask2::new(1, NodeResult::Success), |In(_)| 0.2),
            pair_node_scorer_fn(TesterTask3::new(1, NodeResult::Failure), |In(_)| 0.4),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 1
        app.update(); // 2
        app.update(); // 3
        app.update(); // 0, sequence complete
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 3,
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
        assert!(
            found == &expected,
            "RandomOrderedForcedSequence should match result. found: {:?}",
            found
        );
    }

    #[test]
    fn test_random_forced_selector() {
        let mut app = App::new();
        let rng_res = RngResource::<_, RngMarker>::new(rand::rngs::StdRng::seed_from_u64(224));
        app.insert_resource(rng_res);
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomForcedSelector::new::<rand::rngs::StdRng, RngMarker>(vec![
            pair_node_scorer_fn(TesterTask0::new(1, NodeResult::Failure), |In(_)| 0.1),
            pair_node_scorer_fn(TesterTask1::new(1, NodeResult::Failure), |In(_)| 0.3),
            pair_node_scorer_fn(TesterTask2::new(1, NodeResult::Success), |In(_)| 0.2),
            pair_node_scorer_fn(TesterTask3::new(1, NodeResult::Failure), |In(_)| 0.4),
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
                task_id: 1,
                updated_count: 0,
                frame: 1,
            }],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomForcedSelector should match result. found: {:?}",
            found
        );
    }
}
