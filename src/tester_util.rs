use crate::{
    BehaviorTree, BehaviorTreePlugin,
    node::{DelegateNode, prelude::*},
    task::{TaskBridge, TaskStatus},
};
use bevy::diagnostic::{FrameCount, FrameCountPlugin};

use bevy::prelude::*;

pub mod prelude {
    pub use super::{TestLog, TestLogEntry, TesterPlugin, TesterTask};
    pub use crate::prelude::*;
    pub use bevy::prelude::*;
}

pub struct TesterPlugin;
impl Plugin for TesterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameCountPlugin)
            .add_systems(Update, update::<0>)
            .add_systems(Update, update::<1>)
            .add_systems(Update, update::<2>)
            .add_systems(Update, update::<3>)
            .add_systems(Update, update::<4>)
            .add_systems(Update, update::<5>)
            .add_systems(Update, update::<6>)
            .add_systems(Update, update::<7>)
            .init_resource::<TestLog>();
    }
}

/// Returns result after count.
pub struct TesterTask<const ID: i32> {
    task: TaskBridge,
}
impl<const ID: i32> DelegateNode for TesterTask<ID> {
    fn delegate_node(&self) -> &dyn crate::node::Node {
        &self.task
    }
}
impl<const ID: i32> TesterTask<ID> {
    pub fn new(count: u32, result: NodeResult) -> Self {
        let checker = move |In(entity): In<Entity>, param: Query<&TesterComponent<ID>>| {
            let comp = param.get(entity).expect("TesterComponent not found!");
            if comp.updated_count < count {
                TaskStatus::Running
            } else {
                TaskStatus::Complete(result)
            }
        };
        let task = TaskBridge::new(checker)
            .insert_while_running(TesterComponent::<ID> { updated_count: 0 });
        Self { task }
    }
}

#[derive(Debug, Component, Clone, Copy)]
pub struct TesterComponent<const ID: i32> {
    pub updated_count: u32,
}

#[derive(Debug, Resource, Default, PartialEq, Eq)]
pub struct TestLog {
    pub log: Vec<TestLogEntry>,
}
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct TestLogEntry {
    pub task_id: u32,
    pub updated_count: u32,
    pub frame: u32,
}

fn update<const ID: i32>(
    mut log: ResMut<TestLog>,
    mut comps: Query<&mut TesterComponent<ID>>,
    frame: Res<FrameCount>,
) {
    for mut comp in comps.iter_mut() {
        log.log.push(TestLogEntry {
            task_id: ID as u32,
            updated_count: comp.updated_count,
            frame: frame.0,
        });
        comp.updated_count += 1;
    }
}

#[test]
fn test_enter_tester_task() {
    let mut app = App::new();
    app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
    let task = TesterTask::<0>::new(1, NodeResult::Success);
    let entity = app.world_mut().spawn(BehaviorTree::new(task)).id();
    app.update();
    assert!(
        app.world().get::<TesterComponent<0>>(entity).is_some(),
        "TesterComponent should added on enter."
    );
    // complete the task not to call abort()
    app.update();
}

#[test]
fn test_exit_tester_task() {
    let mut app = App::new();
    app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
    let task = TesterTask::<0>::new(1, NodeResult::Success);
    let entity = app.world_mut().spawn(BehaviorTree::new(task)).id();
    app.update();
    app.update();
    assert!(
        app.world().get::<TesterComponent<0>>(entity).is_none(),
        "TesterComponent should removed on exit."
    );
}

#[test]
fn test_log_test_task() {
    let mut app = App::new();
    app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
    let task = TesterTask::<0>::new(1, NodeResult::Success);
    let _entity = app.world_mut().spawn(BehaviorTree::new(task)).id();
    app.update();
    app.update();
    let expected = TestLog {
        log: vec![TestLogEntry {
            task_id: 0,
            updated_count: 0,
            frame: 1,
        }],
    };
    let found = app.world().get_resource::<TestLog>().unwrap();
    assert!(
        found == &expected,
        "TesterComponent should removed on exit. found: {:?}",
        found
    );
}
