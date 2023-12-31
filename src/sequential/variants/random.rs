
use std::{cmp::Reverse, ops::DerefMut, sync::{Arc, Mutex}};
use bevy::prelude::*;
use ordered_float::OrderedFloat;
use rand::{distributions::Uniform, Rng, prelude::Distribution};

use crate::{Node, NodeGen, NodeResult};
use crate::nullable_access::NullableWorldAccess;
use crate::sequential::{ScoredSequence, NodeScorer, };

use super::last_result;


/// Weighted random sampling.
/// Probability of being picked next is proportional to the score.
/// Using algorithm called A-ES by Efraimidis and Spirakis.
pub fn pick_random_sorted<R: Rng>(mut nodes: Vec<(f32, Arc<dyn Node>)>, rng: &mut R) -> Vec<(f32, Arc<dyn Node>)> {
    let dist = Uniform::<f32>::new(0.0, 1.0);
    nodes.sort_by_key(|(score, _)| Reverse(OrderedFloat(
        dist.sample(rng).powf(1.0/score)
    )));
    nodes
}
/// Weighted random sampling.
/// Note: does not match with first element of `pick_random_sorted`.
pub fn pick_random_one<R: Rng>(nodes: Vec<(f32, Arc<dyn Node>)>, rng: &mut R) -> Vec<(f32, Arc<dyn Node>)> {
    let dist = Uniform::<f32>::new(0.0, 1.0);
    let picked = nodes.into_iter().max_by_key(|(score, _)| OrderedFloat(
        dist.sample(rng).powf(1.0/score)
    ));
    match picked {
        Some(node) => vec![node],
        None => vec![],
    }
}


/// Node that runs children while their result is Success.
/// Children are sorted random weighted by score on enter the node.
pub struct RandomOrderedSequentialAnd{
    delegate: Arc<ScoredSequence>,
}
impl Node for RandomOrderedSequentialAnd {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl RandomOrderedSequentialAnd
{
    pub fn new<R>(node_scorers: Vec<Box<dyn NodeScorer>>, rng: Arc<Mutex<R>>) -> Arc<Self>
    where
        R: Rng + 'static + Send + Sync,
    {
        Arc::new(Self {delegate: ScoredSequence::new(
            node_scorers,
            move |nodes| pick_random_sorted(nodes, (&mut rng.lock().unwrap()).deref_mut()),
            |res| res==NodeResult::Success,
            |_| NodeResult::Success,
        )})
    }
}

/// Node that runs children while their result is Failure.
/// Children are sorted random weighted by score on enter the node.
pub struct RandomOrderedSequentialOr{
    delegate: Arc<ScoredSequence>,
}
impl Node for RandomOrderedSequentialOr {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl RandomOrderedSequentialOr
{
    pub fn new<R>(node_scorers: Vec<Box<dyn NodeScorer>>, rng: Arc<Mutex<R>>) -> Arc<Self>
    where
        R: Rng + 'static + Send + Sync,
    {
        Arc::new(Self {delegate: ScoredSequence::new(
            node_scorers,
            move |nodes| pick_random_sorted(nodes, (&mut rng.lock().unwrap()).deref_mut()),
            |res| res==NodeResult::Failure,
            last_result,
        )})
    }
}
/// Node that runs all children.
/// Children are sorted random weighted by score on enter the node.
pub struct RandomOrderedForcedSequence{
    delegate: Arc<ScoredSequence>,
}
impl Node for RandomOrderedForcedSequence {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl RandomOrderedForcedSequence
{
    pub fn new<R>(node_scorers: Vec<Box<dyn NodeScorer>>, rng: Arc<Mutex<R>>) -> Arc<Self>
    where
        R: Rng + 'static + Send + Sync,
    {
        Arc::new(Self {delegate: ScoredSequence::new(
            node_scorers,
            move |nodes| pick_random_sorted(nodes, (&mut rng.lock().unwrap()).deref_mut()),
            |_| true,
            last_result,
        )})
    }
}

/// Node that runs just one child picked with score-weighted random on enter the node.
pub struct RandomForcedSelector {
    delegate: Arc<ScoredSequence>,
}
impl Node for RandomForcedSelector {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl RandomForcedSelector {
    pub fn new<R>(node_scorers: Vec<Box<dyn NodeScorer>>, rng: Arc<Mutex<R>>) -> Arc<Self>
    where
        R: Rng + 'static + Send + Sync,
    {
        Arc::new(Self {delegate: ScoredSequence::new(
            node_scorers,
            move |nodes| pick_random_one(nodes, (&mut rng.lock().unwrap()).deref_mut()),
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
    use crate::sequential::NodeScorerImpl;
    use super::*;

    use rand::SeedableRng;

    #[test]
    fn test_random_ordered_sequential_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = RandomOrderedSequentialAnd::new(
            vec![
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.1,
                    TesterTask::<0>::new(1, TaskState::Failure)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.3,
                    TesterTask::<1>::new(1, TaskState::Success)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.2,
                    TesterTask::<2>::new(1, TaskState::Success)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.4,
                    TesterTask::<3>::new(1, TaskState::Success)
                )),
            ],
            Arc::new(Mutex::new(rand::rngs::StdRng::seed_from_u64(224)))
        );
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 3
        app.update();  // 2
        app.update();  // 0, sequence complete with Failure
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 3},
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
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.1,
                    TesterTask::<0>::new(1, TaskState::Success)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.3,
                    TesterTask::<1>::new(1, TaskState::Failure)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.2,
                    TesterTask::<2>::new(1, TaskState::Failure)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.4,
                    TesterTask::<3>::new(1, TaskState::Failure)
                )),
            ],
            Arc::new(Mutex::new(rand::rngs::StdRng::seed_from_u64(224)))
        );
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 3
        app.update();  // 2
        app.update();  // 0, sequence complete with Success
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 3},
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
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.1,
                    TesterTask::<0>::new(1, TaskState::Failure)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.3,
                    TesterTask::<1>::new(1, TaskState::Failure)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.2,
                    TesterTask::<2>::new(1, TaskState::Success)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.4,
                    TesterTask::<3>::new(1, TaskState::Failure)
                )),
            ],
            Arc::new(Mutex::new(rand::rngs::StdRng::seed_from_u64(224)))
        );
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 3
        app.update();  // 2
        app.update();  // 0
        app.update();  // 1, sequence complete
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 0, frame: 3},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 4},
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
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.1,
                    TesterTask::<0>::new(1, TaskState::Failure)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.3,
                    TesterTask::<1>::new(1, TaskState::Failure)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 0.2,
                    TesterTask::<2>::new(1, TaskState::Success)
                )),
                Box::new(NodeScorerImpl::new(
                    |In(_)| 10.4,
                    TesterTask::<3>::new(1, TaskState::Failure)
                )),
            ],
            Arc::new(Mutex::new(rand::rngs::StdRng::seed_from_u64(224)))
        );
        let tree = BehaviorTree::new(sequence);
        let _entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 3, sequence complete
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
        ]};
        let found =app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomForcedSelector should match result. found: {:?}", found
        );
    }

}