use std::{ops::DerefMut, sync::{Arc, Mutex}};

use bevy::ecs::{world::World, entity::Entity};
use rand::{distributions::Uniform, Rng, prelude::Distribution};

use crate::node::prelude::*;
use super::{ScoredSequence, Scorer, last_result};
use super::sorted::{pick_sorted, pick_max};


pub mod prelude {
    pub use super::{
        pick_random_sorted, pick_random_one,
        RandomOrderedSequentialAnd,
        RandomOrderedSequentialOr,
        RandomOrderedForcedSequence,
        RandomForcedSelector,
    };
}


/// Weighted random sampling.
/// Probability of being picked next is proportional to the score.
/// Using algorithm called A-ES by Efraimidis and Spirakis.
pub fn pick_random_sorted(scores: Vec<f32>, rng: &mut impl Rng) -> Vec<usize> {
    let dist = Uniform::<f32>::new(0.0, 1.0);
    let scores = scores.into_iter().map(|score|
        dist.sample(rng).powf(1.0/score)
    ).collect();
    pick_sorted(scores)
}
/// Weighted random sampling.
pub fn pick_random_one(scores: Vec<f32>, rng: &mut impl Rng) -> Vec<usize> {
    let dist = Uniform::<f32>::new(0.0, 1.0);
    let scores = scores.into_iter().map(|score|
        dist.sample(rng).powf(1.0/score)
    ).collect();
    pick_max(scores)
}



/// Node that runs children while their result is Success.
/// Children are sorted random weighted by score on enter the node.
pub struct RandomOrderedSequentialAnd{
    delegate: ScoredSequence,
}
impl RandomOrderedSequentialAnd
{
    pub fn new<R>(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>, rng: Arc<Mutex<R>>) -> Self
    where
    R: Rng + 'static + Send + Sync,
    {
        Self {delegate: ScoredSequence::new(
            nodes,
            move |scores| pick_random_sorted(scores, (&mut rng.lock().unwrap()).deref_mut()),
            |res| res==NodeResult::Success,
            |results| if results.is_empty() {NodeResult::Success} else {last_result(results)},
        )}
    }
}
impl Node for RandomOrderedSequentialAnd {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
}


/// Node that runs children while their result is Failure.
/// Children are sorted random weighted by score on enter the node.
pub struct RandomOrderedSequentialOr{
    delegate: ScoredSequence,
}
impl RandomOrderedSequentialOr
{
    pub fn new<R>(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>, rng: Arc<Mutex<R>>) -> Self
    where
    R: Rng + 'static + Send + Sync,
    {
        Self {delegate: ScoredSequence::new(
            nodes,
            move |scores| pick_random_sorted(scores, (&mut rng.lock().unwrap()).deref_mut()),
            |res| res==NodeResult::Failure,
            last_result,
        )}
    }
}
impl Node for RandomOrderedSequentialOr {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
}


/// Node that runs all children.
/// Children are sorted random weighted by score on enter the node.
pub struct RandomOrderedForcedSequence{
    delegate: ScoredSequence,
}
impl RandomOrderedForcedSequence
{
    pub fn new<R>(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>, rng: Arc<Mutex<R>>) -> Self
    where
    R: Rng + 'static + Send + Sync,
    {
        Self {delegate: ScoredSequence::new(
            nodes,
            move |scores| pick_random_sorted(scores, (&mut rng.lock().unwrap()).deref_mut()),
            |_| true,
            last_result,
        )}
    }
}
impl Node for RandomOrderedForcedSequence {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
}


/// Node that runs just one child picked with score-weighted random on enter the node.
pub struct RandomForcedSelector {
    delegate: ScoredSequence,
}
impl RandomForcedSelector {
    pub fn new<R>(nodes: Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)>, rng: Arc<Mutex<R>>) -> Self
    where
    R: Rng + 'static + Send + Sync,
    {
        Self {delegate: ScoredSequence::new(
            nodes,
            move |scores| pick_random_one(scores, (&mut rng.lock().unwrap()).deref_mut()),
            |_| false,
            last_result,
        )}
    }
}
impl Node for RandomForcedSelector {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
}



#[cfg(test)]
mod tests {
    use crate::tester_util::prelude::*;
    use super::*;

    use rand::SeedableRng;

    #[test]
    fn test_random_ordered_sequential_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomOrderedSequentialAnd::new(
            vec![
                pair_node_scorer_fn(
                    TesterTask::<0>::new(1, NodeResult::Success),
                    |In(_)| 0.1,
                ),
                pair_node_scorer_fn(
                    TesterTask::<1>::new(1, NodeResult::Success),
                    |In(_)| 0.3,
                ),
                pair_node_scorer_fn(
                    TesterTask::<2>::new(1, NodeResult::Success),
                    |In(_)| 0.2,
                ),
                pair_node_scorer_fn(
                    TesterTask::<3>::new(1, NodeResult::Failure),
                    |In(_)| 0.4,
                ),
            ],
            Arc::new(Mutex::new(rand::rngs::StdRng::seed_from_u64(224)))
        );
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(sequence)).id();
        app.update();
        app.update();  // 1
        app.update();  // 2
        app.update();  // 3, sequence complete with Failure
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 1, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 3, updated_count: 0, frame: 3},
        ]};
        let found =app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomOrderedSequentialAnd should match result. found: {:?}", found
        );
    }

    #[test]
    fn test_random_ordered_sequential_or() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomOrderedSequentialOr::new(
            vec![
                pair_node_scorer_fn(
                    TesterTask::<0>::new(1, NodeResult::Failure),
                    |In(_)| 0.1,
                ),
                pair_node_scorer_fn(
                    TesterTask::<1>::new(1, NodeResult::Failure),
                    |In(_)| 0.3,
                ),
                pair_node_scorer_fn(
                    TesterTask::<2>::new(1, NodeResult::Failure),
                    |In(_)| 0.2,
                ),
                pair_node_scorer_fn(
                    TesterTask::<3>::new(1, NodeResult::Success),
                    |In(_)| 0.4,
                ),
            ],
            Arc::new(Mutex::new(rand::rngs::StdRng::seed_from_u64(224)))
        );
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(sequence)).id();
        app.update();
        app.update();  // 1
        app.update();  // 2
        app.update();  // 3, sequence complete with Success
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 1, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 3, updated_count: 0, frame: 3},
        ]};
        let found =app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomOrderedSequentialOr should match result. found: {:?}", found
        );
    }

    #[test]
    fn test_random_ordered_forced_sequence() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomOrderedForcedSequence::new(
            vec![
                pair_node_scorer_fn(
                    TesterTask::<0>::new(1, NodeResult::Failure),
                    |In(_)| 0.1,
                ),
                pair_node_scorer_fn(
                    TesterTask::<1>::new(1, NodeResult::Failure),
                    |In(_)| 0.3,
                ),
                pair_node_scorer_fn(
                    TesterTask::<2>::new(1, NodeResult::Success),
                    |In(_)| 0.2,
                ),
                pair_node_scorer_fn(
                    TesterTask::<3>::new(1, NodeResult::Failure),
                    |In(_)| 0.4,
                ),
            ],
            Arc::new(Mutex::new(rand::rngs::StdRng::seed_from_u64(224)))
        );
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(sequence)).id();
        app.update();
        app.update();  // 1
        app.update();  // 2
        app.update();  // 3
        app.update();  // 0, sequence complete
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 1, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 3, updated_count: 0, frame: 3},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 4},
        ]};
        let found =app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomOrderedForcedSequence should match result. found: {:?}", found
        );
    }

    #[test]
    fn test_random_forced_selector() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomForcedSelector::new(
            vec![
                pair_node_scorer_fn(
                    TesterTask::<0>::new(1, NodeResult::Failure),
                    |In(_)| 0.1,
                ),
                pair_node_scorer_fn(
                    TesterTask::<1>::new(1, NodeResult::Failure),
                    |In(_)| 0.3,
                ),
                pair_node_scorer_fn(
                    TesterTask::<2>::new(1, NodeResult::Success),
                    |In(_)| 0.2,
                ),
                pair_node_scorer_fn(
                    TesterTask::<3>::new(1, NodeResult::Failure),
                    |In(_)| 0.4,
                ),
            ],
            Arc::new(Mutex::new(rand::rngs::StdRng::seed_from_u64(224)))
        );
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(sequence)).id();
        app.update();
        app.update();  // 3, sequence complete
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 1, updated_count: 0, frame: 1},
        ]};
        let found =app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomForcedSelector should match result. found: {:?}", found
        );
    }

}
