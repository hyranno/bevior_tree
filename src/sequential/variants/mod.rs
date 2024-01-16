use std::sync::Mutex;

use bevy::ecs::{system::{IntoSystem, In}, entity::Entity};

use crate::node::prelude::*;
use super::{Scorer, ScoredSequence};


pub mod sorted;

#[cfg(feature = "random")]
pub mod random;


pub mod prelude {
    pub use super::{
        score_uniform, pick_identity, last_result,
        SequentialAnd, Sequence,
        SequentialOr, Selector,
        ForcedSequence,
        sorted::prelude::*,
        random::prelude::*,
    };
}


pub fn score_uniform(nodes: Vec<Box<dyn Node>>) -> Vec<(Box<dyn Node>, Mutex<Box<dyn Scorer>>)> {
    fn score(_: In<Entity>) -> f32 {1.0}
    nodes.into_iter().map(
        |node| {
            let scorer: Box<dyn Scorer> = Box::new(IntoSystem::into_system(score));
            (node, Mutex::new(scorer))
        }
    ).collect()
}

pub fn pick_identity(scores: Vec<f32>) -> Vec<usize> {
    (0..scores.len()).collect()
}

pub fn last_result(results: Vec<NodeResult>) -> NodeResult {
    *results.last().unwrap_or(&NodeResult::Failure)
}


pub type Sequence = SequentialAnd;
/// Node that runs children in order while their result is Success.
pub struct SequentialAnd {
    delegate: ScoredSequence,
}
impl SequentialAnd {
    pub fn new(nodes: Vec<Box<dyn Node>>,) -> Self {
        Self {delegate: ScoredSequence::new(
            score_uniform(nodes),
            pick_identity,
            |res| res==NodeResult::Success,
            |_| NodeResult::Success,
        )}
    }
}
impl Node for SequentialAnd {
    fn begin(&self, world: &mut bevy::prelude::World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut bevy::prelude::World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
    fn force_exit(&self, world: &mut bevy::prelude::World, entity: Entity, state: Box<dyn NodeState>) {
        self.delegate.force_exit(world, entity, state)
    }
}


pub type Selector = SequentialOr;
/// Node that runs children in order until one of them returns Success.
pub struct SequentialOr {
    delegate: ScoredSequence,
}
impl SequentialOr {
    pub fn new(nodes: Vec<Box<dyn Node>>,) -> Self {
        Self {delegate: ScoredSequence::new(
            score_uniform(nodes),
            pick_identity,
            |res| res==NodeResult::Failure,
            last_result,
        )}
    }
}
impl Node for SequentialOr {
    fn begin(&self, world: &mut bevy::prelude::World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut bevy::prelude::World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
    fn force_exit(&self, world: &mut bevy::prelude::World, entity: Entity, state: Box<dyn NodeState>) {
        self.delegate.force_exit(world, entity, state)
    }
}


/// Node that runs all children in order.
pub struct ForcedSequence {
    delegate: ScoredSequence,
}
impl ForcedSequence {
    pub fn new(nodes: Vec<Box<dyn Node>>,) -> Self {
        Self {delegate: ScoredSequence::new(
            score_uniform(nodes),
            pick_identity,
            |_| true,
            last_result,
        )}
    }
}
impl Node for ForcedSequence {
    fn begin(&self, world: &mut bevy::prelude::World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut bevy::prelude::World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
    fn force_exit(&self, world: &mut bevy::prelude::World, entity: Entity, state: Box<dyn NodeState>) {
        self.delegate.force_exit(world, entity, state)
    }
}



#[cfg(test)]
mod tests {
    use crate::tester_util::prelude::*;
    use super::*;

    #[test]
    fn test_sequential_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let sequence = Sequence::new(vec![
            Box::new(TesterTask::<0>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<1>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<2>::new(1, NodeResult::Failure)),
            Box::new(TesterTask::<3>::new(1, NodeResult::Success))
        ]);
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(sequence)).id();
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
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "SequentialAnd should match result. found: {:?}", found
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
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(sequence)).id();
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
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "SequentialOr should match result. found: {:?}", found
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
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(sequence)).id();
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
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "ForcedSequence should run all the tasks. found: {:?}", found
        );
    }

}