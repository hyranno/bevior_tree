use crate as bevior_tree;
use crate::node::prelude::*;

use super::Parallel;
use crate::sequential::variants::{result_and, result_or};

pub mod prelude {
    pub use super::{Join, ParallelAnd, ParallelOr};
}

/// Node that runs children in parallel.
/// When one of the children completed with Failure,
///  abort the rest and returns Failure.
#[delegate_node(delegate)]
pub struct ParallelAnd {
    delegate: Parallel,
}
impl ParallelAnd {
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            delegate: Parallel::new(nodes, result_and),
        }
    }
}

/// Node that runs children in parallel.
/// When one of the children completed with Success,
///  abort the rest and returns Success.
#[delegate_node(delegate)]
pub struct ParallelOr {
    delegate: Parallel,
}
impl ParallelOr {
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            delegate: Parallel::new(nodes, result_or),
        }
    }
}

/// Node that runs children in parallel.
/// Complete with Success when all of the children completed.
#[delegate_node(delegate)]
pub struct Join {
    delegate: Parallel,
}
impl Join {
    pub fn new(nodes: Vec<Box<dyn Node>>) -> Self {
        Self {
            delegate: Parallel::new(nodes, |results: Vec<Option<NodeResult>>| {
                if results.contains(&None) {
                    None
                } else {
                    Some(NodeResult::Success)
                }
            }),
        }
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
        let _entity = app
            .world_mut()
            .spawn(BehaviorTreeBundle::from_root(parallel))
            .id();
        app.update();
        app.update(); // 0, 1, 2, 3
        app.update(); // 1, 2, 3
        app.update(); // 2, 3, completed with Failure
        app.update(); // nop
                      // Order of the log entries within same frame may change.
        let expected: HashSet<TestLogEntry> = vec![
            TestLogEntry {
                task_id: 0,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 1,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 1,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 2,
                frame: 3,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 2,
                frame: 3,
            },
        ]
        .into_iter()
        .collect();
        let found: HashSet<TestLogEntry> = app
            .world()
            .get_resource::<TestLog>()
            .unwrap()
            .log
            .clone()
            .into_iter()
            .collect();
        assert!(
            found == expected,
            "ParallelAnd should match result. found: {:?}",
            found
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
        let _entity = app
            .world_mut()
            .spawn(BehaviorTreeBundle::from_root(parallel))
            .id();
        app.update();
        app.update(); // 0, 1, 2, 3
        app.update(); // 1, 2, 3
        app.update(); // 2, 3, complete with Success
        app.update(); // nop
                      // Order of the log entries within same frame may change.
        let expected: HashSet<TestLogEntry> = vec![
            TestLogEntry {
                task_id: 0,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 1,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 1,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 2,
                frame: 3,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 2,
                frame: 3,
            },
        ]
        .into_iter()
        .collect();
        let found: HashSet<TestLogEntry> = app
            .world()
            .get_resource::<TestLog>()
            .unwrap()
            .log
            .clone()
            .into_iter()
            .collect();
        assert!(
            found == expected,
            "ParallelOr should match result. found: {:?}",
            found
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
        let _entity = app
            .world_mut()
            .spawn(BehaviorTreeBundle::from_root(parallel))
            .id();
        app.update();
        app.update(); // 0, 1, 2, 3
        app.update(); // 1, 2, 3
        app.update(); // 2, 3
        app.update(); // 3, parallel completed
        app.update(); // nop
                      // Order of the log entries within same frame may change.
        let expected: HashSet<TestLogEntry> = vec![
            TestLogEntry {
                task_id: 0,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 1,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 0,
                frame: 1,
            },
            TestLogEntry {
                task_id: 1,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 1,
                frame: 2,
            },
            TestLogEntry {
                task_id: 2,
                updated_count: 2,
                frame: 3,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 2,
                frame: 3,
            },
            TestLogEntry {
                task_id: 3,
                updated_count: 3,
                frame: 4,
            },
        ]
        .into_iter()
        .collect();
        let found: HashSet<TestLogEntry> = app
            .world()
            .get_resource::<TestLog>()
            .unwrap()
            .log
            .clone()
            .into_iter()
            .collect();
        assert!(
            found == expected,
            "Join should match result. found: {:?}",
            found
        );
    }
}
