use std::sync::{Arc, Mutex};
use bevy::prelude::*;

use crate::{Node, NodeGen, NodeResult, NodeGenState};
use crate::nullable_access::NullableWorldAccess;
use super::Parallel;


/// Node that runs children in parallel.
/// When one of the children completed with Failure,
///  abort the rest and returns Failure.
pub struct ParallelAnd {
    delegate: Arc<Parallel>,
}
impl Node for ParallelAnd {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl ParallelAnd {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: Parallel::new(
            nodes,
            |states: Vec<&NodeGenState>| {
                if states.contains(&&NodeGenState::Complete(NodeResult::Failure)) {
                    NodeGenState::Complete(NodeResult::Failure)
                } else if states.contains(&&NodeGenState::Yielded(())) {
                    NodeGenState::Yielded(())
                } else {
                    NodeGenState::Complete(NodeResult::Success)
                }
            },
        )})
    }
}

/// Node that runs children in parallel.
/// When one of the children completed with Success,
///  abort the rest and returns Success.
pub struct ParallelOr {
    delegate: Arc<Parallel>,
}
impl Node for ParallelOr {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl ParallelOr {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: Parallel::new(
            nodes,
            |states: Vec<&NodeGenState>| {
                if states.contains(&&NodeGenState::Complete(NodeResult::Success)) {
                    NodeGenState::Complete(NodeResult::Success)
                } else if states.contains(&&NodeGenState::Yielded(())) {
                    NodeGenState::Yielded(())
                } else {
                    NodeGenState::Complete(NodeResult::Failure)
                }
            },
        )})
    }
}


/// Node that runs children in parallel.
/// Complete with Success when all of the children completed.
pub struct Join {
    delegate: Arc<Parallel>,
}
impl Node for Join {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen> {
        self.delegate.clone().run(world, entity)
    }
}
impl Join {
    pub fn new(nodes: Vec<Arc<dyn Node>>,) -> Arc<Self> {
        Arc::new(Self {delegate: Parallel::new(
            nodes,
            |states: Vec<&NodeGenState>| {
                if states.contains(&&NodeGenState::Yielded(())) {
                    NodeGenState::Yielded(())
                } else {
                    NodeGenState::Complete(NodeResult::Success)
                }
            },
        )})
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::*;
    use crate::task::*;
    use crate::tester_util::{TesterPlugin, TesterTask, TestLog, TestLogEntry};
    use super::*;

    #[test]
    fn test_abort() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let parallel = Join::new(vec![
            TesterTask::<0>::new(1, TaskState::Success),
            TesterTask::<1>::new(2, TaskState::Success),
            TesterTask::<2>::new(3, TaskState::Failure),
            TesterTask::<3>::new(4, TaskState::Success),
        ]);
        let tree = BehaviorTree::new(parallel);
        let entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 0, 1, 2, 3
        app.world.entity_mut(entity).insert(Abort);
        app.update();  // 1, 2, 3, tree abort
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
        ].into_iter().collect();
        let found: HashSet<TestLogEntry> = app.world.get_resource::<TestLog>().unwrap().log.clone().into_iter().collect();
        assert!(
            found == expected,
            "Parallel should be able to abort. found: {:?}", found
        );
    }

    #[test]
    fn test_and() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let parallel = ParallelAnd::new(vec![
            TesterTask::<0>::new(1, TaskState::Success),
            TesterTask::<1>::new(2, TaskState::Success),
            TesterTask::<2>::new(3, TaskState::Failure),
            TesterTask::<3>::new(4, TaskState::Success),
        ]);
        let tree = BehaviorTree::new(parallel);
        let _entity = app.world.spawn(tree).id();
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
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let parallel = ParallelOr::new(vec![
            TesterTask::<0>::new(1, TaskState::Failure),
            TesterTask::<1>::new(2, TaskState::Failure),
            TesterTask::<2>::new(3, TaskState::Success),
            TesterTask::<3>::new(4, TaskState::Failure),
        ]);
        let tree = BehaviorTree::new(parallel);
        let _entity = app.world.spawn(tree).id();
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
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let parallel = Join::new(vec![
            TesterTask::<0>::new(1, TaskState::Success),
            TesterTask::<1>::new(2, TaskState::Success),
            TesterTask::<2>::new(3, TaskState::Failure),
            TesterTask::<3>::new(4, TaskState::Success),
        ]);
        let tree = BehaviorTree::new(parallel);
        let _entity = app.world.spawn(tree).id();
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