//! Behavior tree plugin for Bevy.

use bevy::{
    ecs::{intern::Interned, schedule::ScheduleLabel},
    prelude::*,
};

pub mod conditional;
pub mod converter;
pub mod node;
pub mod parallel;
pub mod sequential;
pub mod task;

#[cfg(test)]
mod tester_util;

use node::{Node, NodeStatus};

/// Module for convenient imports. Use with `use bevior_tree::prelude::*;`.
pub mod prelude {
    #[cfg(feature = "serde")]
    pub use crate::BehaviorTreeSource;
    pub use crate::{
        BehaviorTree, BehaviorTreePlugin, BehaviorTreeRoot, BehaviorTreeSystemSet, Freeze,
        TreeStatus, conditional::prelude::*, converter::prelude::*, node::prelude::*,
        parallel::prelude::*, sequential::prelude::*, task::prelude::*,
    };
}

/// Add to your app to use this crate.
pub struct BehaviorTreePlugin {
    schedule: Interned<dyn ScheduleLabel>,
}
impl BehaviorTreePlugin {
    /// Adds the systems to the given schedule rather than default [`PostUpdate`].
    pub fn in_schedule(mut self, schedule: impl ScheduleLabel) -> Self {
        self.schedule = schedule.intern();
        self
    }
}
impl Default for BehaviorTreePlugin {
    fn default() -> Self {
        Self {
            schedule: PostUpdate.intern(),
        }
    }
}
impl Plugin for BehaviorTreePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assets<BehaviorTreeRoot>>().add_systems(
            self.schedule,
            (update).in_set(BehaviorTreeSystemSet::Update),
        );
        #[cfg(feature = "serde")]
        {
            app.init_asset::<BehaviorTreeRoot>()
                .add_systems(PreUpdate, load_from_source);
        }
    }
}

/// SystemSet that the plugin use.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, SystemSet)]
pub enum BehaviorTreeSystemSet {
    Update,
}

/// Asset representing behavior tree root node.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Asset, TypePath)]
pub struct BehaviorTreeRoot {
    node: Box<dyn Node>,
}

/// Component to specify the source path of the behavior tree asset.
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize, Component, Clone)]
pub struct BehaviorTreeSource {
    pub path: String,
}

/// Behavior tree component.
/// Nodes of the tree receive the entity with this component.
#[derive(Component, Clone)]
#[require(TreeStatus)]
pub struct BehaviorTree {
    root: Handle<BehaviorTreeRoot>,
}
impl BehaviorTree {
    pub fn new(root: Handle<BehaviorTreeRoot>) -> Self {
        Self { root }
    }
    pub fn from_node<N: Node>(node: N, asset_server: &mut Assets<BehaviorTreeRoot>) -> Self {
        let handle = asset_server.add(BehaviorTreeRoot {
            node: Box::new(node),
        });
        Self { root: handle }
    }
    pub fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        world.resource_scope(|world, assets: Mut<Assets<BehaviorTreeRoot>>| {
            assets
                .get(&self.root)
                .map(|root| root.node.as_ref().begin(world, entity))
                .unwrap_or(NodeStatus::Beginning)
        })
    }
    pub fn resume(
        &self,
        world: &mut World,
        entity: Entity,
        state: Box<dyn node::NodeState>,
    ) -> NodeStatus {
        world.resource_scope(|world, assets: Mut<Assets<BehaviorTreeRoot>>| {
            match assets.get(&self.root) {
                None => NodeStatus::Pending(state),
                Some(root) => root.node.as_ref().resume(world, entity, state),
            }
        })
    }
}

/// Add to the same entity with the BehaviorTree to temporarily freeze the update.
/// You may prefer [`conditional::ElseFreeze`] node.
/// Freezes transition of the tree, not running task.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Freeze;

/// Represents the state of the tree.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Component)]
pub struct TreeStatus(NodeStatus);

impl Default for TreeStatus {
    fn default() -> Self {
        Self(NodeStatus::Beginning)
    }
}

/// The system to update the states of the behavior trees attached to entities.
fn update(
    world: &mut World,
    query: &mut QueryState<(Entity, &BehaviorTree), (With<TreeStatus>, Without<Freeze>)>,
) {
    let targets = query
        .iter(world)
        .map(|(entity, tree)| (entity, tree.clone()))
        .collect::<Vec<_>>();
    targets.into_iter().for_each(|(entity, tree)| {
        if let Some(TreeStatus(status)) = world.entity_mut(entity).take::<TreeStatus>() {
            let new_status = match status {
                NodeStatus::Beginning => tree.begin(world, entity),
                NodeStatus::Pending(state) => tree.resume(world, entity, state),
                NodeStatus::Complete(_) => status,
            };
            world.entity_mut(entity).insert(TreeStatus(new_status));
        }
    });
}

/// System to load behavior tree assets from source paths.
/// Attach `BehaviorTreeSource` component to an entity to trigger loading.
#[cfg(feature = "serde")]
pub fn load_from_source(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &BehaviorTreeSource), (Added<BehaviorTreeSource>, Without<BehaviorTree>)>,
) {
    for (entity, source) in query.iter() {
        let handle = asset_server.load(&source.path);
        commands
            .entity(entity)
            .insert(BehaviorTree { root: handle });
    }
}

#[cfg(test)]
mod tests {
    use crate::{node::NodeStatus, tester_util::prelude::*};

    #[test]
    fn test_tree_end_with_result() {
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
        let status = app.world().get::<TreeStatus>(entity);
        assert!(
            status.is_some(),
            "BehaviorTree should have result on the end."
        );
        assert!(
            if let Some(TreeStatus(NodeStatus::Complete(result))) = status {
                result == &NodeResult::Success
            } else {
                false
            },
            "BehaviorTree should have result that match with the result of the root."
        );
    }

    #[test]
    fn test_freeze() {
        let mut app = App::new();
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let task = TesterTask0::new(2, NodeResult::Success);
        let tree = BehaviorTree::from_node(
            task,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let entity = app.world_mut().spawn(tree).id();
        app.update();
        app.world_mut().entity_mut(entity).insert(Freeze);
        app.update(); // 0
        app.update(); // 1
        app.update(); // 2
        app.world_mut().entity_mut(entity).remove::<Freeze>();
        app.update(); // 3, task complete
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 0,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 1,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 2,
                    frame: 3,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 3,
                    frame: 4,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Task should not proceed while freeze. found: {:?}",
            found
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_save_and_load_roundtrip() {
        use std::fs;
        use std::path::Path;

        let task = TesterTask0::new(1, NodeResult::Success);
        let root = BehaviorTreeRoot {
            node: Box::new(task),
        };

        // Save the asset to a temporary file
        let test_asset_dir = Path::new("target").join("test_assets");
        fs::create_dir_all(&test_asset_dir).expect("Failed to create test directory");
        let file_name = "roundtrip_tree.ron";
        let file_path = test_asset_dir.join(file_name);
        let ron = ron::to_string(&root).expect("Failed to serialize test asset");
        fs::write(&file_path, ron).expect("Failed to write test asset");

        let mut app = App::new();
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));

        let source = BehaviorTreeSource {
            path: file_name.to_string(),
        };
        let entity = app.world_mut().spawn(source).id();

        // Allow some frames for asset loading and processing
        for _ in 0..50 {
            app.update();
        }

        app.world_mut()
            .resource_scope(|world, asset_server: Mut<AssetServer>| {
                if let Some(tree) = world.query::<&BehaviorTree>().iter(world).next() {
                    let load_state = asset_server.get_load_state(&tree.root);
                    println!("LoadState = {:?}", load_state);
                }
            });

        let status = app.world().get::<TreeStatus>(entity);
        assert!(
            status.is_some(),
            "BehaviorTree should have result on the end."
        );
        assert!(
            if let Some(TreeStatus(NodeStatus::Complete(result))) = status {
                println!("Found TreeStatus with result: {:?}", result);
                result == &NodeResult::Success
            } else {
                if let Some(TreeStatus(s)) = status {
                    match s {
                        NodeStatus::Beginning => println!("TreeStatus is still Beginning"),
                        NodeStatus::Pending(_) => println!("TreeStatus is still Pending"),
                        NodeStatus::Complete(r) => {
                            println!("TreeStatus is Complete with result: {:?}", r)
                        }
                    }
                }
                false
            },
            "BehaviorTree should have result that match with the result of the root."
        );
    }
}
