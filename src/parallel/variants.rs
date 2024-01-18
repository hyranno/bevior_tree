
use bevy::ecs::{entity::Entity, world::World};

use crate::node::prelude::*;

use super::Parallel;


pub mod prelude {
    pub use super::{
        ParallelAnd, ParallelOr, Join,
    };
}


/// Node that runs children in parallel.
/// When one of the children completed with Failure,
///  abort the rest and returns Failure.
pub struct ParallelAnd {
    delegate: Parallel,
}
impl ParallelAnd {
    pub fn new(nodes: Vec<Box<dyn Node>>,) -> Self {
        Self {delegate: Parallel::new(
            nodes,
            |results: Vec<Option<NodeResult>>| {
                if results.contains(&Some(NodeResult::Failure)) {
                    Some(NodeResult::Failure)
                } else if results.contains(&None) {
                    None
                } else {
                    Some(NodeResult::Success)
                }
            },
        )}
    }
}
impl Node for ParallelAnd {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        self.delegate.force_exit(world, entity, state)
    }
}

/// Node that runs children in parallel.
/// When one of the children completed with Success,
///  abort the rest and returns Success.
pub struct ParallelOr {
    delegate: Parallel,
}
impl ParallelOr {
    pub fn new(nodes: Vec<Box<dyn Node>>,) -> Self {
        Self {delegate: Parallel::new(
            nodes,
            |results: Vec<Option<NodeResult>>| {
                if results.contains(&Some(NodeResult::Success)) {
                    Some(NodeResult::Success)
                } else if results.contains(&None) {
                    None
                } else {
                    Some(NodeResult::Failure)
                }
            },
        )}
    }
}
impl Node for ParallelOr {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        self.delegate.force_exit(world, entity, state)
    }
}


/// Node that runs children in parallel.
/// Complete with Success when all of the children completed.
pub struct Join {
    delegate: Parallel,
}
impl Join {
    pub fn new(nodes: Vec<Box<dyn Node>>,) -> Self {
        Self {delegate: Parallel::new(
            nodes,
            |results: Vec<Option<NodeResult>>| {
                if results.contains(&None) {
                    None
                } else {
                    Some(NodeResult::Success)
                }
            },
        )}
    }
}
impl Node for Join {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
    fn force_exit(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) {
        self.delegate.force_exit(world, entity, state)
    }
}



#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::tester_util::prelude::*;

    #[test]
    fn test_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let parallel = ParallelAnd::new(vec![
            Box::new(TesterTask::<0>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<1>::new(2, NodeResult::Success)),
            Box::new(TesterTask::<2>::new(3, NodeResult::Failure)),
            Box::new(TesterTask::<3>::new(4, NodeResult::Success)),
        ]);
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(parallel)).id();
        app.update();
        app.update();  // 0, 1, 2, 3
        app.update();  // 1, 2, 3
        app.update();  // 2, 3, completed with Failure
        app.update();  // nop
        // Order of the log entries within same frame may change.
        let expected: HashSet<TestLogEntry> = vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 3, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 2, frame: 3},
            TestLogEntry {task_id: 3, updated_count: 2, frame: 3},
        ].into_iter().collect();
        let found: HashSet<TestLogEntry> = app.world.get_resource::<TestLog>().unwrap().log.clone().into_iter().collect();
        assert!(
            found == expected,
            "ParallelAnd should match result. found: {:?}", found
        );
    }

    #[test]
    fn test_or() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let parallel = ParallelOr::new(vec![
            Box::new(TesterTask::<0>::new(1, NodeResult::Failure)),
            Box::new(TesterTask::<1>::new(2, NodeResult::Failure)),
            Box::new(TesterTask::<2>::new(3, NodeResult::Success)),
            Box::new(TesterTask::<3>::new(4, NodeResult::Failure)),
        ]);
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(parallel)).id();
        app.update();
        app.update();  // 0, 1, 2, 3
        app.update();  // 1, 2, 3
        app.update();  // 2, 3, complete with Success
        app.update();  // nop
        // Order of the log entries within same frame may change.
        let expected: HashSet<TestLogEntry> = vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 3, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 2, frame: 3},
            TestLogEntry {task_id: 3, updated_count: 2, frame: 3},
        ].into_iter().collect();
        let found: HashSet<TestLogEntry> = app.world.get_resource::<TestLog>().unwrap().log.clone().into_iter().collect();
        assert!(
            found == expected,
            "ParallelOr should match result. found: {:?}", found
        );
    }

    #[test]
    fn test_join() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let parallel = Join::new(vec![
            Box::new(TesterTask::<0>::new(1, NodeResult::Success)),
            Box::new(TesterTask::<1>::new(2, NodeResult::Success)),
            Box::new(TesterTask::<2>::new(3, NodeResult::Failure)),
            Box::new(TesterTask::<3>::new(4, NodeResult::Success)),
        ]);
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(parallel)).id();
        app.update();
        app.update();  // 0, 1, 2, 3
        app.update();  // 1, 2, 3
        app.update();  // 2, 3
        app.update();  // 3, parallel completed
        app.update();  // nop
        // Order of the log entries within same frame may change.
        let expected: HashSet<TestLogEntry> = vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 2, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 3, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 1, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 3, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 2, updated_count: 2, frame: 3},
            TestLogEntry {task_id: 3, updated_count: 2, frame: 3},
            TestLogEntry {task_id: 3, updated_count: 3, frame: 4},
        ].into_iter().collect();
        let found: HashSet<TestLogEntry> = app.world.get_resource::<TestLog>().unwrap().log.clone().into_iter().collect();
        assert!(
            found == expected,
            "Join should match result. found: {:?}", found
        );
    }

}
