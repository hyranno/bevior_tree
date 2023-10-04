// In this game, the player moves around in 2D with the arrow keys, but if they get too close to the
// enemy, the enemy moves towards them, until the player moves back out of range.
// This example matches to the one of `seldom_state`.

use bevy::prelude::*;
use bevior_tree::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, BehaviorTreePlugin::default()))
        // This plugin is required for `bevior_tree`
        .add_systems(Startup, init)
        .add_systems(Update, (follow, move_player))
        .run();
}

// Setup the game
fn init(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    // Simple player entity
    let player = commands
        .spawn((
            SpriteBundle {
                texture: asset_server.load("player.png"),
                ..default()
            },
            Player,
        ))
        .id();

    // The enemy
    commands.spawn((
        SpriteBundle {
            transform: Transform::from_xyz(500., 0., 0.),
            texture: asset_server.load("enemy.png"),
            ..default()
        },

        // This behavior tree handles the enemy's behavior.
        BehaviorTree::new(
            ConditionalLoop::new(
                Sequence::new(vec![
                    // TaskImpl can be used as DoNothingTask.
                    Arc::new(TaskImpl::new(NearTaskChecker {target: player, range: 300.})),
                    // Task to follow the player.
                    FollowTask::new(player, 300., 100.),
                ]),
                Always
            )
        ),
    ));
}


// This task checker runs until the entity gets within the the given range of the target.
struct NearTaskChecker {
    target: Entity,
    range: f32,
}
impl TaskChecker for NearTaskChecker {
    // Put the parameters that your checker needs here. `Param` is read-only; you may not access
    // system params that write to the `World`. `Time` is included here to demonstrate how to get
    // multiple system params.
    type Param<'w, 's> = (Query<'w, 's, &'static Transform>, Res<'w, Time>);
    fn check (
        &self,
        entity: Entity,
        (transforms, _time): Self::Param<'_, '_>,
    ) -> TaskState {
        // Find the distance between the target and this entity
        let distance = transforms
            .get(self.target)
            .unwrap()
            .translation
            .truncate()
            .distance(transforms.get(entity).unwrap().translation.truncate());

        // Check whether the target is within range. If it is, return `Success`.
        match distance <= self.range {
            true => TaskState::Success,
            false => TaskState::Running,
        } 
    }
}


// This task checker will fail when the target is out of range.
struct FollowTaskChecker {
    target: Entity,
    range: f32,
}
impl TaskChecker for FollowTaskChecker {
    type Param<'w, 's> = Query<'w, 's, &'static Transform>;
    fn check (
        &self,
        entity: Entity,
        transforms: Self::Param<'_, '_>,
    ) -> TaskState {
        let distance = transforms
            .get(self.target)
            .unwrap()
            .translation
            .truncate()
            .distance(transforms.get(entity).unwrap().translation.truncate());

        // Return `Failure` if it is out of range.
        match distance <= self.range {
            true => TaskState::Running,
            false => TaskState::Failure,
        } 
    }
}

// Task node to follow the target.
// Task trait is available for making your task, delegating core methods to TaskImpl.
struct FollowTask {
    task: Arc<TaskImpl<<Self as Task>::Checker>>,
}
impl FollowTask {
    pub fn new(
        target: Entity,
        range: f32,
        speed: f32,
    ) -> Arc<Self> {
        let task = TaskImpl::new(FollowTaskChecker {target, range})
            // Task inserts some components to the entity while running.
            .insert_while_running(Follow {target, speed})
            // Or run some commands on enter/exit.
            .on_enter(|_entity, mut _commands| { info!("Beginning to follow."); })
        ;
        Arc::new(Self {
            task: Arc::new(task),
        })
    }
}
impl Task for FollowTask {
    // Specify what checker you use with this task.
    type Checker = FollowTaskChecker;
    fn task_impl(&self) -> Arc<TaskImpl<Self::Checker>> {
        self.task.clone()
    }
}

// Entities in the `Follow` task move toward the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
struct Follow {
    target: Entity,
    speed: f32,
}

// Let's define some real behavior for entities in the follow task.
fn follow(
    mut transforms: Query<&mut Transform>,
    follows: Query<(Entity, &Follow)>,
    time: Res<Time>,
) {
    for (entity, follow) in &follows {
        // Get the positions of the follower and target
        let target_translation = transforms.get(follow.target).unwrap().translation;
        let follow_transform = &mut transforms.get_mut(entity).unwrap();
        let follow_translation = follow_transform.translation;

        // Find the direction from the follower to the target and go that way
        follow_transform.translation += (target_translation - follow_translation)
            .normalize_or_zero()
            * follow.speed
            * time.delta_seconds();
    }
}


// The code after this comment is not related to `bevior_tree`. It's just player movement.

#[derive(Component)]
struct Player;

const PLAYER_SPEED: f32 = 200.;

fn move_player(
    mut players: Query<&mut Transform, With<Player>>,
    keys: Res<Input<KeyCode>>,
    time: Res<Time>,
) {
    players.single_mut().translation += Vec3::new(
        (keys.pressed(KeyCode::Right) as i32 - keys.pressed(KeyCode::Left) as i32) as f32,
        (keys.pressed(KeyCode::Up) as i32 - keys.pressed(KeyCode::Down) as i32) as f32,
        0.,
    )
    .normalize_or_zero()
        * PLAYER_SPEED
        * time.delta_seconds();
}
