//! Behavior tree plugin for Bevy.

use std::{sync::{Arc, Mutex}, future::Future};
use bevy::prelude::*;
use genawaiter::{sync::{Co, Gen}, GeneratorState};

use self::nullable_access::{NullableWorldAccess, TemporalWorldSharing};

pub mod task;
pub mod sequential;
pub mod parallel;
pub mod conditional;
pub mod converter;

mod nullable_access;

#[cfg(test)]
mod tester_util;

/// Module for convenient imports. Use with `use bevior_tree::prelude::*;`.
pub mod prelude {
    pub use std::sync::Arc;
    pub use crate::{
        *,
        task::*,
        sequential::variants::*,
        conditional::{ConditionalLoop, ConditionChecker, variants::*},
        converter::{ResultConverter, variants::*},
    };
}


/// Add to your app to use this crate
pub struct BehaviorTreePlugin;
impl Plugin for BehaviorTreePlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(PostUpdate, update)
        ;
    }
}

/// Behavior tree component.
/// Task nodes of the tree affect the entity with this component.
#[derive(Component)]
pub struct BehaviorTree {
    root: Arc<dyn Node>,
    runner: Option<NodeRunner>,
    result: Option<NodeResult>,
    world: Arc<Mutex<NullableWorldAccess>>,
}
/// Add to the same entity with the BehaviorTree to temporarily freeze the update.
#[derive(Component)]
pub struct Freeze;
/// Add to the same entity with the BehaviorTree to abort the process.
/// You should abort before remove the BehaviorTree, or on_exit of the running task will not be executed.
#[derive(Component)]
pub struct Abort;

impl BehaviorTree {
    pub fn new(root: Arc<dyn Node>) -> Self {
        Self {
            root,
            runner: None,
            result: None,
            world: Arc::<Mutex::<NullableWorldAccess>>::default(),
        }
    }
    pub fn result(&self) -> Option<NodeResult> {
        self.result
    }
    fn stub(&self) -> Self {
        Self {
            root: Arc::<StubNode>::default(),
            runner: None,
            result: None,
            world: Arc::default(),
        }
    }
}
impl Drop for BehaviorTree {
    fn drop(&mut self) {
        if let Some(gen) = self.runner.as_mut() {
            gen.abort_if_incomplete();
        }
    }
}

fn update (
    world: &mut World,
    query: &mut QueryState<(Entity, &mut BehaviorTree, Option<&Abort>), Without<Freeze>>,
) {
    // Pull the trees out of the world so we can invoke mutable methods on them.
    let mut borrowed_trees: Vec<(Entity, BehaviorTree, bool)> = query.iter_mut(world)
        .map(|(entity, mut tree, abort)| {
            let stub = tree.stub();
            (entity, std::mem::replace(tree.as_mut(), stub), abort.is_some())
        })
        .collect()
    ;

    for (entity, tree, abort) in borrowed_trees.iter_mut() {
        if tree.result.is_some() { continue; }
        let _temporal_world = TemporalWorldSharing::new(tree.world.clone(), world);

        match tree.runner.as_mut() {
            None => {   // Not started yet.
                if !*abort {
                    tree.runner = Some(NodeRunner::new(tree.root.clone(), tree.world.clone(), *entity));
                } else {
                    tree.result = Some(NodeResult::Aborted);
                }
            },
            Some(runner) => {   // Running.
                if !*abort {
                    runner.resume_if_incomplete();
                } else {
                    runner.abort_if_incomplete();
                }
                tree.result = runner.result();
            },
        }

        // Drop used runner.
        if tree.result.is_some() {
            tree.runner = None;
        }
    }

    // put the borrowed trees back
    for (entity, tree, _) in borrowed_trees {
        *query.get_mut(world, entity).unwrap().1 = tree;
    }
}

/// Nodes return this on complete.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NodeResult {
    Success,
    Failure,
    Aborted,
}
impl Into<bool> for NodeResult {
    fn into(self) -> bool {
        match self {
            NodeResult::Success => true,
            NodeResult::Failure => false,
            _ => {warn!("converted {:?} into bool", self); false}
        }
    }
}
impl From<bool> for NodeResult {
    fn from(value: bool) -> Self {
        if value { NodeResult::Success } else { NodeResult::Failure }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResumeSignal {
    Resume,
    Abort,
}

type Y = ();
type R = ResumeSignal;
type C = NodeResult;
pub type NodeGenState = GeneratorState<Y, C>;

/// Representation of one execution of the node.
/// In the implementation, it is generator function.
pub trait NodeGen: Send + Sync {
    fn resume(&mut self) -> NodeGenState;
    fn abort(&mut self) -> NodeGenState;
}
impl<F> NodeGen for Gen<Y, R, F> where
F: Future<Output = C> + Send + Sync + 'static
{
    fn resume(&mut self) -> NodeGenState {
        Gen::<Y, R, F>::resume_with(self, ResumeSignal::Resume)
    }
    fn abort(&mut self) -> NodeGenState {
        Gen::<Y, R, F>::resume_with(self, ResumeSignal::Abort)
    }
}

/// Node of behavior tree.
pub trait Node: Send + Sync {
    fn run(self: Arc<Self>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Box<dyn NodeGen>;
}

#[derive(Debug, Default)]
struct StubNode;
impl Node for StubNode {
    fn run(self: Arc<Self>, _world: Arc<Mutex<NullableWorldAccess>>, _entity: Entity) -> Box<dyn NodeGen> {
        let producer = |_co| async move {
            NodeResult::Failure
        };
        Box::new(Gen::new(producer))
    }
}


/// Container for `NodeGen` and its result.
pub struct NodeRunner {
    gen: Box<dyn NodeGen>,
    state: NodeGenState,
}
impl NodeRunner {
    pub fn new(node: Arc<dyn Node>, world: Arc<Mutex<NullableWorldAccess>>, entity: Entity) -> Self {
        let mut gen = node.run(world, entity);
        let state = gen.resume();
        Self { gen, state }
    }
    pub fn state(&self) -> &NodeGenState {
        &self.state
    }
    pub fn result(&self) -> Option<NodeResult> {
        match self.state {
            NodeGenState::Yielded(()) => None,
            NodeGenState::Complete(res) => Some(res)
        }
    }
    pub fn resume_if_incomplete(&mut self) {
        if self.result().is_none() {
            self.state = self.gen.resume();
        }
    }
    pub fn abort_if_incomplete(&mut self) {
        if self.result().is_none() {
            self.state = self.gen.abort();
        }
    }
}


async fn complete_or_yield(co: &Co<(), ResumeSignal>, gen: &mut Box<dyn NodeGen>) -> NodeResult {
    let mut state = gen.resume();
    loop {
        match state {
            NodeGenState::Yielded(yielded_value) => {
                let signal = co.yield_(yielded_value).await;
                if signal == ResumeSignal::Abort {
                    gen.abort();
                    return NodeResult::Aborted;
                }
                state = gen.resume();
            },
            NodeGenState::Complete(result) => { return result; }
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::tester_util::*;

    #[test]
    fn test_tree_end_with_result() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = TesterTask::<0>::new(1, TaskState::Success);
        let tree = BehaviorTree::new(task);
        let entity = app.world.spawn(tree).id();
        app.update();
        app.update();
        let tree = app.world.get::<BehaviorTree>(entity).unwrap();
        assert!(
            tree.result.is_some(),
            "BehaviorTree should have result on the end."
        );
        assert!(
            tree.result.unwrap() == NodeResult::Success,
            "BehaviorTree should have result that match with the result of the root."
        );
        assert!(
            tree.runner.is_none(),
            "BehaviorTree shold not have generator after the run."
        );
    }

    #[test]
    fn test_freeze() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = TesterTask::<0>::new(2, TaskState::Success);
        let tree = BehaviorTree::new(task);
        let entity = app.world.spawn(tree).id();
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
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "Task should not proceed while freeze."
        );
    }

    #[test]
    fn test_abort() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = TesterTask::<0>::new(4, TaskState::Success);
        let tree = BehaviorTree::new(task);
        let entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 0
        app.world.entity_mut(entity).insert(Abort);
        app.update();  // 1, tree abort
        app.update();
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 0, updated_count: 1, frame: 2},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "BehaviorTree should be aborted."
        );
    }

    #[test]
    fn test_drop() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin, TesterPlugin));
        let task = TesterTask::<0>::new(4, TaskState::Success);
        let tree = BehaviorTree::new(task);
        let entity = app.world.spawn(tree).id();
        app.update();
        app.update();  // 0
        app.world.entity_mut(entity).remove::<BehaviorTree>();
        app.update();  // 1, BehaviorTree still exists on Update stage.
        app.update();  // 2, Dropping BehaviorTree with running task will not execute on_exit 
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
            TestLogEntry {task_id: 0, updated_count: 1, frame: 2},
            TestLogEntry {task_id: 0, updated_count: 2, frame: 3},
        ]};
        assert!(
            app.world.get_resource::<TestLog>().unwrap() == &expected,
            "Dropped BehaviorTree should be aborted."
        );
    }

}
