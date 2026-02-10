use crate::{
    BehaviorTree, BehaviorTreePlugin, BehaviorTreeRoot,
    node::prelude::*,
    task::{
        TaskBridge, TaskChecker, TaskDefinition, TaskEvent, TaskEventListener, TaskStatus,
        insert_while_running,
    },
};
use bevy::diagnostic::FrameCount;

use bevy::prelude::*;

pub mod prelude {
    pub use super::{
        TestLog, TestLogEntry, TesterPlugin, TesterTask0, TesterTask1, TesterTask2, TesterTask3,
    };
    pub use crate::prelude::*;
    pub use bevy::prelude::*;
}

pub struct TesterPlugin;
impl Plugin for TesterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, update::<0>)
            .add_systems(Update, update::<1>)
            .add_systems(Update, update::<2>)
            .add_systems(Update, update::<3>)
            .add_systems(Update, update::<4>)
            .add_systems(Update, update::<5>)
            .add_systems(Update, update::<6>)
            .add_systems(Update, update::<7>)
            .init_resource::<TestLog>();
        #[cfg(feature = "serde")]
        {
            let test_asset_dir = std::path::Path::new("target").join("test_assets");
            app.add_plugins((
                AssetPlugin {
                    file_path: test_asset_dir.to_string_lossy().to_string(),
                    ..default()
                },
                bevy_common_assets::ron::RonAssetPlugin::<BehaviorTreeRoot>::new(&["ron"]),
            ));
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TesterTaskDef<const ID: i32> {
    pub count: u32,
    pub result: NodeResult,
}

macro_rules! define_tester_node {
    (
        $id:expr,
        $wrapper_name:ident,
        $def_tag_name:literal
    ) => {
        #[cfg_attr(feature = "serde", typetag::serde(name = $def_tag_name))]
        impl TaskDefinition for TesterTaskDef<$id> {
            fn build_checker(&self) -> Box<dyn TaskChecker> {
                let count = self.count;
                let result = self.result;
                Box::new(IntoSystem::into_system(
                    move |In(entity), param: Query<&TesterComponent<$id>>| {
                        let comp = param
                            .get(entity)
                            .expect(concat!("TesterComponent not found for ID ", $id));
                        if comp.updated_count < count {
                            TaskStatus::Running
                        } else {
                            TaskStatus::Complete(result)
                        }
                    },
                ))
            }

            fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<dyn TaskEventListener>)> {
                insert_while_running(TesterComponent::<$id> { updated_count: 0 })
            }
        }

        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $wrapper_name {
            task: TaskBridge,
        }
        #[cfg_attr(feature = "serde", typetag::serde)]
        impl crate::node::Node for $wrapper_name {
            fn begin(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
            ) -> crate::node::NodeStatus {
                self.task.begin(world, entity)
            }
            fn resume(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                state: Box<dyn crate::node::NodeState>,
            ) -> crate::node::NodeStatus {
                self.task.resume(world, entity, state)
            }
            fn force_exit(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                state: Box<dyn crate::node::NodeState>,
            ) {
                self.task.force_exit(world, entity, state)
            }
        }

        impl $wrapper_name {
            pub fn new(count: u32, result: NodeResult) -> Self {
                let def = TesterTaskDef::<$id> { count, result };
                Self {
                    task: TaskBridge::new(Box::new(def)),
                }
            }
        }
    };
}
define_tester_node!(0, TesterTask0, "TesterTaskDef0");
define_tester_node!(1, TesterTask1, "TesterTaskDef1");
define_tester_node!(2, TesterTask2, "TesterTaskDef2");
define_tester_node!(3, TesterTask3, "TesterTaskDef3");

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
    app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
    let task = TesterTask0::new(1, NodeResult::Success);
    let tree = BehaviorTree::from_node(
        task,
        &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
    );
    let entity = app.world_mut().spawn(tree).id();
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
    app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
    let task = TesterTask0::new(1, NodeResult::Success);
    let tree = BehaviorTree::from_node(
        task,
        &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
    );
    let entity = app.world_mut().spawn(tree).id();
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
    app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
    let task = TesterTask0::new(1, NodeResult::Success);
    let tree = BehaviorTree::from_node(
        task,
        &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
    );
    let _entity = app.world_mut().spawn(tree).id();
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
