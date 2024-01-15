

use std::borrow::Cow;

use bevy::ecs::{system::{IntoSystem, ReadOnlySystem, In, System, Combine, CombinatorSystem}, entity::Entity, world::World};

use crate::node::prelude::*;
use super::{ConditionalLoop, LoopState};


pub mod prelude {
    pub use super::{
        SeparableConditionChecker,
        Conditional,
    };
}


pub type SeparableConditionChecker<A, B> = CombinatorSystem<SeparableConditionCheckerMarker, A, B>;
pub struct SeparableConditionCheckerMarker;
impl<A, B> Combine<A,B> for SeparableConditionCheckerMarker
where
    A: System<In=Entity, Out=bool>,
    B: System<In=LoopState, Out=bool>,
{
    type In = (Entity, LoopState);
    type Out = bool;
    fn combine(
        (entity, loop_state): Self::In,
        a: impl FnOnce(<A as System>::In) -> <A as System>::Out,
        b: impl FnOnce(<B as System>::In) -> <B as System>::Out,
    ) -> Self::Out {
        b(loop_state) && a(entity)
    }
}



/// Node that runs the child if condition is matched.
pub struct Conditional {
    delegate: ConditionalLoop,
}
impl Conditional {
    pub fn new<F, Marker>(child:  impl Node, checker: F) -> Self
    where
        F: IntoSystem<Entity, bool, Marker>,
        <F as IntoSystem<Entity, bool, Marker>>::System : ReadOnlySystem,
    {
        Self { delegate: ConditionalLoop::new(
            child,
            SeparableConditionChecker::new(
                IntoSystem::into_system(checker),
                IntoSystem::into_system(|In(loop_state): In<LoopState>|
                    loop_state.count < 1 && loop_state.last_result.is_none() // only once
                ),
                Cow::Borrowed("check cond")
            )
        )}
    }
}
impl Node for Conditional {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
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
        let _entity = app.world.spawn(BehaviorTreeBundle::from_root(conditional)).id();
        app.update();
        app.update();  // nop
        let expected = TestLog {log: vec![
        ]};
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Conditional should not do the task. Found {:?}", found
        );
    }

    #[test]
    fn test_conditional_true() {
        let mut app = App::new();
        app.add_plugins((BehaviorTreePlugin::default(), TesterPlugin));
        let task = TesterTask::<0>::new(1, NodeResult::Success);
        let conditional = Conditional::new(task, test_marker_exists);
        let _entity = app.world.spawn((BehaviorTreeBundle::from_root(conditional), TestMarker)).id();
        app.update();
        app.update();  // 0
        app.update();  // nop
        let expected = TestLog {log: vec![
            TestLogEntry {task_id: 0, updated_count: 0, frame: 1},
        ]};
        let found = app.world.get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "Conditional should do the task. Found {:?}", found
        );
    }
}


