use bevy::{
    ecs::{
        entity::Entity,
        system::{
            CombinatorSystem, Combine, In, IntoSystem, ReadOnlySystem, RunSystemError, System,
            SystemInput,
        },
    },
    utils::prelude::DebugName,
};

use super::{ConditionalLoop, LoopState};
use crate as bevior_tree;
use crate::node::prelude::*;

pub mod prelude {
    pub use super::{Conditional, SeparableConditionChecker};
}

pub type SeparableConditionChecker<A, B> = CombinatorSystem<SeparableConditionCheckerMarker, A, B>;
pub struct SeparableConditionCheckerMarker;
impl<A, B> Combine<A, B> for SeparableConditionCheckerMarker
where
    A: System<In = In<Entity>, Out = bool>,
    B: System<In = In<LoopState>, Out = bool>,
{
    type In = In<(Entity, LoopState)>;
    type Out = bool;
    fn combine<T>(
        (entity, loop_state): <Self::In as SystemInput>::Inner<'_>,
        data: &mut T,
        a: impl FnOnce(
            <<A as System>::In as SystemInput>::Inner<'_>,
            &mut T,
        ) -> Result<A::Out, RunSystemError>,
        b: impl FnOnce(
            <<B as System>::In as SystemInput>::Inner<'_>,
            &mut T,
        ) -> Result<A::Out, RunSystemError>,
    ) -> Result<Self::Out, RunSystemError> {
        Ok(b(loop_state, data)? && a(entity, data)?)
    }
}

/// Node that runs the child once if condition is matched.
#[delegate_node(delegate)]
pub struct Conditional {
    delegate: ConditionalLoop,
}
impl Conditional {
    pub fn new<F, Marker>(child: impl Node, checker: F) -> Self
    where
        F: IntoSystem<In<Entity>, bool, Marker>,
        <F as IntoSystem<In<Entity>, bool, Marker>>::System: ReadOnlySystem,
    {
        Self {
            delegate: ConditionalLoop::new(
                child,
                SeparableConditionChecker::new(
                    IntoSystem::into_system(checker),
                    IntoSystem::into_system(
                        |In(loop_state): In<LoopState>| {
                            loop_state.count < 1 && loop_state.last_result.is_none()
                        }, // only once
                    ),
                    DebugName::borrowed("check cond"),
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tester_util::prelude::*;

    #[derive(Component)]
    struct TestMarker;

    fn test_marker_exists(In(entity): In<Entity>, world: &World) -> bool {
        world.entity(entity).contains::<TestMarker>()
    }

    #[test]
    fn test_conditional_false() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let conditional = Conditional::new(task, test_marker_exists);
        let tree = BehaviorTree::from_node(
            conditional,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // nop
        let expected = TestLog { log: vec![] };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Conditional should not do the task. Found {:?}",
            found
        );
    }

    #[test]
    fn test_conditional_true() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let conditional = Conditional::new(task, test_marker_exists);
        let tree = BehaviorTree::from_node(
            conditional,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn((tree, TestMarker)).id();
        app.update();
        app.update(); // 0
        app.update(); // nop
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
            "Conditional should do the task. Found {:?}",
            found
        );
    }
}
