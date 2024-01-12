//! Behavior tree plugin for Bevy.

use std::sync::Arc;
use bevy::{prelude::*, ecs::schedule::ScheduleLabel};


pub mod node;
pub mod task;

#[cfg(test)]
mod tester_util;

use node::{Node, NodeStatus, NodeState};

/// Module for convenient imports. Use with `use bevior_tree::prelude::*;`.
pub mod prelude {
    pub use crate::{
        BehaviorTreePlugin, BehaviorTreeSystemSet,
        BehaviorTree, Freeze, TreeStatus,
        node::prelude::*,
        task::prelude::*,
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
    use crate::tester_util::prelude::*;
}
