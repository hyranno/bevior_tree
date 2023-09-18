
use bevy::core::FrameCount;
pub use bevy::prelude::*;
pub use crate::prelude::*;

pub struct TesterPlugin;
impl Plugin for TesterPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(FrameCountPlugin)
            .add_systems(Update, update)
            .init_resource::<TestLog>()
        ;
    }
}

/// Returns result after count.
pub struct TesterTask {
    task: Arc<TaskImpl<<Self as Task>::Checker>>,
}
impl Task for TesterTask {
    type Checker = TesterTaskChecker;
    fn task_impl(&self) -> Arc<TaskImpl<Self::Checker>> {
        self.task.clone()
    }
}
impl TesterTask {
    pub fn new(id: u32, count: u32, result: TaskState) -> Arc<Self> {
        let task = Arc::new(TaskImpl::new(TesterTaskChecker {count, result})
            .insert_while_running(TesterComponent { task_id: id, updated_count: 0 })
        );
        Arc::new(Self {task})
    }
}

pub struct TesterTaskChecker {
    pub count: u32,
    pub result: TaskState,
}
impl TaskChecker for TesterTaskChecker {
    type Param<'w, 's> = Query<'w, 's, &'static TesterComponent>;
    fn check (
        &self,
        entity: Entity,
        comps: Self::Param<'_, '_>,
    ) -> TaskState {
        let Ok(comp) = comps.get(entity) else {
            panic!("TesterComponent not found!");
        };
        if comp.updated_count < self.count {
            TaskState::Running
        } else {
            self.result
        }
    }
}

#[derive(Debug, Component, Clone, Copy)]
pub struct TesterComponent {
    pub task_id: u32,
    pub updated_count: u32,
}

#[derive(Debug, Resource, Default, PartialEq, Eq)]
pub struct TestLog {
    pub log: Vec<TestLogEntry>
}
#[derive(Debug, PartialEq, Eq)]
pub struct TestLogEntry {
    pub task_id: u32,
    pub updated_count: u32,
    pub frame: u32,
}


fn update(
    mut log: ResMut<TestLog>,
    mut comps: Query<&mut TesterComponent>,
    frame: Res<FrameCount>,
) {
    for mut comp in comps.iter_mut() {
        log.log.push(TestLogEntry {
            task_id: comp.task_id,
            updated_count: comp.updated_count,
            frame: frame.0,
        });
        comp.updated_count += 1;
    }
}


#[test]
fn test_enter_tester_task() {
    let mut app = App::new();
    app.add_plugins((BehaviorTreePlugin, TesterPlugin));
    let task = TesterTask::new(0, 1, TaskState::Success);
    let tree = BehaviorTree::new(task);
    let entity = app.world.spawn(tree).id();
    app.update();
    assert!(
        app.world.get::<TesterComponent>(entity).is_some(),
        "TesterComponent should added on enter."
    );
    // complete the task not to call abort()
    app.update();
}

#[test]
fn test_exit_tester_task() {
    let mut app = App::new();
    app.add_plugins((BehaviorTreePlugin, TesterPlugin));
    let task = TesterTask::new(0, 1, TaskState::Success);
    let tree = BehaviorTree::new(task);
    let entity = app.world.spawn(tree).id();
    app.update();
    app.update();
    assert!(
        app.world.get::<TesterComponent>(entity).is_none(),
        "TesterComponent should removed on exit."
    );
}

#[test]
fn test_log_test_task() {
    let mut app = App::new();
    app.add_plugins((BehaviorTreePlugin, TesterPlugin));
    let task = TesterTask::new(0, 1, TaskState::Success);
    let tree = BehaviorTree::new(task);
    let _entity = app.world.spawn(tree).id();
    app.update();
    app.update();
    let expected = TestLog {log: vec![
        TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
    ]};
    assert!(
        app.world.get_resource::<TestLog>().unwrap() == &expected,
        "TesterComponent should removed on exit."
    );
}
