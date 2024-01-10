//! Behavior tree plugin for Bevy.

use std::{sync::{Arc, Mutex}, future::Future};
use bevy::{prelude::*, ecs::schedule::ScheduleLabel};


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

fn update (
) {
}


#[cfg(test)]
mod tests {
}
