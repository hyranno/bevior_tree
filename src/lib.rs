//! Behavior tree plugin for Bevy.

use std::sync::Arc;
use bevy::{prelude::*, ecs::schedule::ScheduleLabel};


pub mod node;
pub mod task;
pub mod sequential;

#[cfg(test)]
mod tester_util;

use node::{Node, NodeStatus, NodeState};

/// Module for convenient imports. Use with `use bevior_tree::prelude::*;`.
pub mod prelude {
    pub use crate::{
        BehaviorTreePlugin, BehaviorTreeSystemSet,
        BehaviorTreeBundle, BehaviorTree, Freeze, TreeStatus,
        node::prelude::*,
        task::prelude::*,
        sequential::prelude::*,
    };
}

/// Add to your app to use this crate.
pub struct BehaviorTreePlugin {
    schedule: Box<dyn ScheduleLabel>,
}
impl BehaviorTreePlugin {
    /// Adds the systems to the given schedule rather than default [`PostUpdate`].
    pub fn in_schedule(mut self, schedule: impl ScheduleLabel) -> Self {
        self.schedule = Box::new(schedule);
        self
    }
}
impl Default for BehaviorTreePlugin {
    fn default() -> Self {
        Self { schedule: Box::new(PostUpdate) }
    }
}
impl Plugin for BehaviorTreePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(PostUpdate, (update).in_set(BehaviorTreeSystemSet::Update))
        ;
    }
}

/// SystemSet that the plugin use.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, SystemSet)]
pub enum BehaviorTreeSystemSet {
    Update,
}


/// Behavior tree component.
/// Task nodes of the tree affect the entity with this component.
#[derive(Component, Clone)]
pub struct BehaviorTree {
    root: Arc<dyn Node>,
}
impl BehaviorTree {
    pub fn new(root: impl Node) -> Self {
        Self { root: Arc::new(root) }
    }
}
impl Node for BehaviorTree {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.root.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.root.resume(world, entity, state)
    }
}

#[derive(Bundle)]
pub struct BehaviorTreeBundle {
    pub tree: BehaviorTree,
    pub status: TreeStatus,
}
impl BehaviorTreeBundle {
    pub fn from_root(root: impl Node) -> Self {
        Self::from_tree(BehaviorTree::new(root))
    }
    pub fn from_tree(tree: BehaviorTree) -> Self {
        Self { tree, status: TreeStatus(NodeStatus::Beginning) }
    }
}

/// Add to the same entity with the BehaviorTree to temporarily freeze the update.
/// You may prefer [`conditional::variants::ElseFreeze`] node.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Freeze;

#[derive(Component)]
pub struct TreeStatus(NodeStatus);

fn update (
    world: &mut World,
    query: &mut QueryState<(Entity, &BehaviorTree, &mut TreeStatus), Without<Freeze>>,
) {
    let trees: Vec<(Entity, Arc<dyn Node>, NodeStatus)> = query.iter_mut(world).map(
        |(entity, tree, mut status)| {
            let mut status_swap = TreeStatus(NodeStatus::Beginning);
            std::mem::swap(status.as_mut(), &mut status_swap);
            (entity, tree.root.clone(), status_swap.0)
        }
    ).collect();

    let statuses_new: Vec<NodeStatus> = trees.into_iter().map(
        |(entity, root, status)| {
            match status {
                NodeStatus::Beginning => root.begin(world, entity),
                NodeStatus::Pending(state) => root.resume(world, entity, state),
                NodeStatus::Complete(_) => status
            }
        }
    ).collect();

    query.iter_mut(world).zip(statuses_new).for_each( |((_, _, mut state), state_new)| {
        let mut state_new_swap = TreeStatus(state_new);
        std::mem::swap(state.as_mut(), &mut state_new_swap);
    });

}


#[cfg(test)]
mod tests {
    use crate::{tester_util::prelude::*, node::NodeStatus};

    #[test]
    fn test_tree_end_with_result() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let entity = app.world.spawn(BehaviorTreeBundle::from_root(task)).id();
        app.update();
        app.update();
        let status = app.world.get::<TreeStatus>(entity);
        assert!(
            status.is_some(),
            "BehaviorTree should have result on the end."
        );
        assert!(
            if let Some(TreeStatus(NodeStatus::Complete(result))) = status {
                result == &NodeResult::Success
            } else {false},
            "BehaviorTree should have result that match with the result of the root."
        );
    }

    #[test]
    fn test_freeze() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(2, NodeResult::Success);
        let entity = app.world.spawn(BehaviorTreeBundle::from_root(task)).id();
        app.update();
        app.world.entity_mut(entity).insert(Freeze);
        app.update();  // 0
        app.update();  // 1
        app.update();  // 2
        app.world.entity_mut(entity).remove::<Freeze>();
        app.update();  // 3, task complete
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 0, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 2, frame: 3},
            TestLogEntry {task_id: 0, updated_count: 3, frame: 4},
        ]};
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Task should not proceed while freeze. found: {:?}", found
        );
    }

}
