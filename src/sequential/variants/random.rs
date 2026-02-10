use std::marker::PhantomData;

use bevy::{
    ecs::system::{In, IntoSystem},
    prelude::{Entity, ResMut, Resource, World},
};

use rand::{Rng, distr::Uniform, prelude::Distribution};

use super::sorted::{MaxPickerBuilder, SortedPickerBuilder};
use super::{
    AndResultStrategy, ForcedResultStrategy, LastResultStrategy, OrResultStrategy, Picker,
    PickerBuilder, ScoredSequence, ScorerBuilder,
};
use crate as bevior_tree;
use crate::node::prelude::*;

pub mod prelude {
    pub use super::{
        RandomForcedSelector, RandomOrderedForcedSequence, RandomOrderedSequentialAnd,
        RandomOrderedSequentialOr, RandomPickerBuilder, RngResource,
    };
    #[cfg(feature = "serde")]
    pub use crate::impl_random_picker;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RandomPickerBuilder<R, Marker>
where
    R: Rng + 'static + Send + Sync,
    Marker: 'static + Send + Sync,
{
    pub base: Box<dyn PickerBuilder>,
    #[cfg_attr(feature = "serde", serde(skip))]
    _phantom: PhantomData<(R, Marker)>,
}
impl<R, Marker> RandomPickerBuilder<R, Marker>
where
    R: Rng + 'static + Send + Sync,
    Marker: 'static + Send + Sync,
{
    pub fn new(base: Box<dyn PickerBuilder>) -> Self {
        Self {
            base,
            _phantom: PhantomData,
        }
    }
    /// Weighted random sampling.
    /// Probability of being picked next is proportional to the score.
    /// Using algorithm called A-ES by Efraimidis and Spirakis.
    fn randomizer(
        In((scores, entity)): In<(Vec<f32>, Entity)>,
        mut rng_res: ResMut<RngResource<R, Marker>>,
    ) -> (Vec<f32>, Entity) {
        let dist = Uniform::<f32>::new(0.0, 1.0).expect("Failed to init uniform distribution.");
        let scores = scores
            .into_iter()
            .map(|score| dist.sample(&mut rng_res.rng).powf(1.0 / score))
            .collect();
        (scores, entity)
    }
    pub fn inner_build(&self) -> Box<dyn Picker> {
        let mut base = self.base.build();
        // Wrap base picker while BoxedSystem does not implement IntoSystem directly.
        let wrapped_base = move |In((scores, entity)): In<(Vec<f32>, Entity)>,
                                 world: &mut World| {
            base.initialize(world);
            let picked = base.run((scores, entity), world);
            picked.expect("Failed to run Picker")
        };
        let randomizer = Self::randomizer;
        let piped = randomizer.pipe(wrapped_base);
        Box::new(IntoSystem::into_system(piped))
    }
}
#[cfg(not(feature = "serde"))]
impl<R, Marker> PickerBuilder for RandomPickerBuilder<R, Marker>
where
    R: Rng + 'static + Send + Sync,
    Marker: 'static + Send + Sync,
{
    fn build(&self) -> Box<dyn Picker> {
        self.inner_build()
    }
}
#[cfg(feature = "serde")]
mod serde_impls {
    /// Implement PickerBuilder for RandomPickerBuilder with given RNG type and Marker type.
    /// This is a macro because typetag does not support generic impl directly.
    #[macro_export]
    macro_rules! impl_random_picker {
        ($rng:ty, $marker:ty) => {
            #[typetag::serde]
            impl PickerBuilder for RandomPickerBuilder<$rng, $marker> {
                fn build(&self) -> Box<dyn Picker> {
                    self.inner_build()
                }
            }
        };
    }
}

/// Resource that holds RNG instance.
/// Insert this resource to use random-based nodes.
#[derive(Resource)]
pub struct RngResource<R, Marker>
where
    R: Rng + 'static + Send + Sync,
    Marker: 'static + Send + Sync,
{
    phantom: PhantomData<Marker>,
    rng: R,
}
impl<R, Marker> RngResource<R, Marker>
where
    R: Rng + 'static + Send + Sync,
    Marker: 'static + Send + Sync,
{
    pub fn new(rng: R) -> Self {
        Self {
            phantom: PhantomData,
            rng,
        }
    }
}

/// Node that runs children while their result is Success.
/// Children are sorted random weighted by score on enter the node.
#[delegate_node(delegate)]
pub struct RandomOrderedSequentialAnd {
    delegate: ScoredSequence,
}
impl RandomOrderedSequentialAnd {
    pub fn new<R, Marker>(children: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>) -> Self
    where
        R: Rng + 'static + Send + Sync,
        Marker: 'static + Send + Sync,
        RandomPickerBuilder<R, Marker>: PickerBuilder,
    {
        Self {
            delegate: ScoredSequence::new(
                children,
                RandomPickerBuilder::<R, Marker>::new(Box::new(SortedPickerBuilder)),
                AndResultStrategy,
            ),
        }
    }
}

/// Node that runs children while their result is Failure.
/// Children are sorted random weighted by score on enter the node.
#[delegate_node(delegate)]
pub struct RandomOrderedSequentialOr {
    delegate: ScoredSequence,
}
impl RandomOrderedSequentialOr {
    pub fn new<R, Marker>(children: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>) -> Self
    where
        R: Rng + 'static + Send + Sync,
        Marker: 'static + Send + Sync,
        RandomPickerBuilder<R, Marker>: PickerBuilder,
    {
        Self {
            delegate: ScoredSequence::new(
                children,
                RandomPickerBuilder::<R, Marker>::new(Box::new(SortedPickerBuilder)),
                OrResultStrategy,
            ),
        }
    }
}

/// Node that runs all children.
/// Children are sorted random weighted by score on enter the node.
#[delegate_node(delegate)]
pub struct RandomOrderedForcedSequence {
    delegate: ScoredSequence,
}
impl RandomOrderedForcedSequence {
    pub fn new<R, Marker>(children: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>) -> Self
    where
        R: Rng + 'static + Send + Sync,
        Marker: 'static + Send + Sync,
        RandomPickerBuilder<R, Marker>: PickerBuilder,
    {
        Self {
            delegate: ScoredSequence::new(
                children,
                RandomPickerBuilder::<R, Marker>::new(Box::new(SortedPickerBuilder)),
                LastResultStrategy,
            ),
        }
    }
}

/// Node that runs just one child picked with score-weighted random on enter the node.
#[delegate_node(delegate)]
pub struct RandomForcedSelector {
    delegate: ScoredSequence,
}
impl RandomForcedSelector {
    pub fn new<R, Marker>(children: Vec<(Box<dyn Node>, Box<dyn ScorerBuilder>)>) -> Self
    where
        R: Rng + 'static + Send + Sync,
        Marker: 'static + Send + Sync,
        RandomPickerBuilder<R, Marker>: PickerBuilder,
    {
        Self {
            delegate: ScoredSequence::new(
                children,
                RandomPickerBuilder::<R, Marker>::new(Box::new(MaxPickerBuilder)),
                ForcedResultStrategy,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ConstantScorerBuilder;
    use super::*;
    use crate::{impl_random_picker, tester_util::prelude::*};

    use rand::SeedableRng;

    struct RngMarker;

    impl_random_picker!(rand::rngs::StdRng, RngMarker);

    #[test]
    fn test_random_ordered_sequential_and() {
        let mut app = App::new();
        let rng_res = RngResource::<_, RngMarker>::new(rand::rngs::StdRng::seed_from_u64(224));
        app.insert_resource(rng_res);
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let sequence = RandomOrderedSequentialAnd::new::<rand::rngs::StdRng, RngMarker>(vec![
            (
                Box::new(TesterTask0::new(1, NodeResult::Success)),
                Box::new(ConstantScorerBuilder { score: 0.1 }),
            ),
            (
                Box::new(TesterTask1::new(1, NodeResult::Success)),
                Box::new(ConstantScorerBuilder { score: 0.3 }),
            ),
            (
                Box::new(TesterTask2::new(1, NodeResult::Success)),
                Box::new(ConstantScorerBuilder { score: 0.2 }),
            ),
            (
                Box::new(TesterTask3::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.4 }),
            ),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 1
        app.update(); // 2
        app.update(); // 3, sequence complete with Failure
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 3,
                    updated_count: 0,
                    frame: 3,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomOrderedSequentialAnd should match result. found: {:?}",
            found
        );
    }

    #[test]
    fn test_random_ordered_sequential_or() {
        let mut app = App::new();
        let rng_res = RngResource::<_, RngMarker>::new(rand::rngs::StdRng::seed_from_u64(224));
        app.insert_resource(rng_res);
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let sequence = RandomOrderedSequentialOr::new::<rand::rngs::StdRng, RngMarker>(vec![
            (
                Box::new(TesterTask0::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.1 }),
            ),
            (
                Box::new(TesterTask1::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.3 }),
            ),
            (
                Box::new(TesterTask2::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.2 }),
            ),
            (
                Box::new(TesterTask3::new(1, NodeResult::Success)),
                Box::new(ConstantScorerBuilder { score: 0.4 }),
            ),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 1
        app.update(); // 2
        app.update(); // 3, sequence complete with Success
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 3,
                    updated_count: 0,
                    frame: 3,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomOrderedSequentialOr should match result. found: {:?}",
            found
        );
    }

    #[test]
    fn test_random_ordered_forced_sequence() {
        let mut app = App::new();
        let rng_res = RngResource::<_, RngMarker>::new(rand::rngs::StdRng::seed_from_u64(224));
        app.insert_resource(rng_res);
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let sequence = RandomOrderedForcedSequence::new::<rand::rngs::StdRng, RngMarker>(vec![
            (
                Box::new(TesterTask0::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.1 }),
            ),
            (
                Box::new(TesterTask1::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.3 }),
            ),
            (
                Box::new(TesterTask2::new(1, NodeResult::Success)),
                Box::new(ConstantScorerBuilder { score: 0.2 }),
            ),
            (
                Box::new(TesterTask3::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.4 }),
            ),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 1
        app.update(); // 2
        app.update(); // 3
        app.update(); // 0, sequence complete
        app.update(); // nop
        let expected = TestLog {
            log: vec![
                TestLogEntry {
                    task_id: 1,
                    updated_count: 0,
                    frame: 1,
                },
                TestLogEntry {
                    task_id: 2,
                    updated_count: 0,
                    frame: 2,
                },
                TestLogEntry {
                    task_id: 3,
                    updated_count: 0,
                    frame: 3,
                },
                TestLogEntry {
                    task_id: 0,
                    updated_count: 0,
                    frame: 4,
                },
            ],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomOrderedForcedSequence should match result. found: {:?}",
            found
        );
    }

    #[test]
    fn test_random_forced_selector() {
        let mut app = App::new();
        let rng_res = RngResource::<_, RngMarker>::new(rand::rngs::StdRng::seed_from_u64(224));
        app.insert_resource(rng_res);
        app.add_plugins((TesterPlugin, BehaviorTreePlugin::default()));
        let sequence = RandomForcedSelector::new::<rand::rngs::StdRng, RngMarker>(vec![
            (
                Box::new(TesterTask0::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.1 }),
            ),
            (
                Box::new(TesterTask1::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.3 }),
            ),
            (
                Box::new(TesterTask2::new(1, NodeResult::Success)),
                Box::new(ConstantScorerBuilder { score: 0.2 }),
            ),
            (
                Box::new(TesterTask3::new(1, NodeResult::Failure)),
                Box::new(ConstantScorerBuilder { score: 0.4 }),
            ),
        ]);
        let tree = BehaviorTree::from_node(
            sequence,
            &mut app.world_mut().resource_mut::<Assets<BehaviorTreeRoot>>(),
        );
        let _entity = app.world_mut().spawn(tree).id();
        app.update();
        app.update(); // 3, sequence complete
        app.update(); // nop
        let expected = TestLog {
            log: vec![TestLogEntry {
                task_id: 1,
                updated_count: 0,
                frame: 1,
            }],
        };
        let found = app.world().get_resource::<TestLog>().unwrap();
        assert!(
            found == &expected,
            "RandomForcedSelector should match result. found: {:?}",
            found
        );
    }
}
