//! Behavior tree plugin for Bevy.

use std::sync::Arc;
use bevy::{prelude::*, ecs::schedule::ScheduleLabel};


pub mod node;

use node::{Node, NodeStatus};

/// Module for convenient imports. Use with `use bevior_tree::prelude::*;`.
pub mod prelude {}

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
#[derive(Component)]
pub struct BehaviorTree {
    root: Arc<dyn Node>,
}
/// Add to the same entity with the BehaviorTree to temporarily freeze the update.
/// You may prefer [`conditional::variants::ElseFreeze`] node.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Freeze;

#[derive(Component)]
pub struct TreeStatus(NodeStatus);

fn update (
    mut query: Query<(&BehaviorTree, &mut TreeStatus), Without<Freeze>>,
) {
    for (tree, mut status) in query.iter_mut() {
        match &status.0 {
            NodeStatus::Beginning => status.0 = tree.root.begin(),
            NodeStatus::Pending(state) => status.0 = tree.root.resume(state),
            NodeStatus::Complete(_) => {},
        };
    }
}


#[cfg(test)]
mod tests {
}
