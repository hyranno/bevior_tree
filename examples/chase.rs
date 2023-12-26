// In this game, the player moves around in 2D with the arrow keys, but if they get too close to the
// enemy, the enemy moves towards them, until the player moves back out of range.
// This example matches to the one of `seldom_state`.

use bevior_tree::prelude::*;
use bevy::prelude::*;
use std::sync::Mutex;

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
    let _ = commands
        .spawn((
            SpriteBundle {
                texture: asset_server.load("player.png"),
                ..default()
            },
            Player::Wasd,
        ))
        .id();
    let _ = commands
        .spawn((
            SpriteBundle {
                texture: asset_server.load("player.png"),
                ..default()
            },
            Player::ArrowKeys,
        ))
        .id();

    // The enemy
    for spawn_pos in [
        Vec2::new(500., 0.),
        Vec2::new(250., 0.),
        Vec2::new(-250., 0.),
        Vec2::new(-500., 0.),
        Vec2::new(500., 100.),
        Vec2::new(250., 100.),
        Vec2::new(-250., 100.),
        Vec2::new(-500., 100.),
        Vec2::new(500., -100.),
        Vec2::new(250., -100.),
        Vec2::new(-250., -100.),
        Vec2::new(-500., -100.),
    ] {
        let near_task_output = Arc::new(Mutex::new(NearTaskOutput { target: None }));
        commands.spawn((
            SpriteBundle {
                transform: Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.),
                texture: asset_server.load("enemy.png"),
                ..default()
            },
            // This behavior tree handles the enemy's behavior.
            BehaviorTree::new(ConditionalLoop::new(
                Sequence::new(vec![
                    // Task to wait until player get near.
                    NearTask::new(300.0, near_task_output.clone()),
                    // Task to follow the player.
                    FollowTask::new(300., 25., move || {
                        let output = near_task_output.lock().unwrap();
                        output.target
                    }),
                ]),
                |In(_)| true,
            )),
        ));
    }
}

// Task to wait until player get near.
// Task trait is available for making your task, delegating core methods to TaskImpl.
task!(NearTask);
pub struct NearTaskOutput {
    pub target: Option<Entity>,
}
impl NearTask {
    pub fn new(range: f32, output: Arc<Mutex<NearTaskOutput>>) -> Arc<Self> {
        let checker =
            move |In(entity): In<Entity>,
                  q_transforms: Query<&Transform>,
                  q_players: Query<(Entity, &Transform), With<Player>>| {
                let entity_transform = q_transforms.get(entity).unwrap();
                let mut output = output.lock().unwrap();
                for (player_entity, player_transform) in q_players.iter() {
                    if player_transform
                        .translation
                        .distance(entity_transform.translation)
                        < range
                    {
                        output.target = Some(player_entity);
                        return TaskState::Success;
                    }
                }
                output.target = None;
                TaskState::Running
            };
        Arc::new(Self {
            task: Arc::new(TaskImpl::new(checker)),
        })
    }
}

// Task node to follow the target.
task!(FollowTask);
impl FollowTask {
    pub fn new<F>(range: f32, speed: f32, target: F) -> Arc<Self>
    where
        F: Fn() -> Option<Entity> + Send + Sync + 'static,
    {
        let checker = move |In(entity): In<Entity>,
                            q_transform: Query<&Transform>,
                            mut q_follow: Query<&mut Follow>| {
            let maybe_target = (target)();
            let mut entity_follow = q_follow.get_mut(entity).unwrap();
            entity_follow.target = maybe_target;
            match maybe_target {
                Some(target) => {
                    let target_transform = q_transform.get(target).unwrap();
                    let entity_transform = q_transform.get(entity).unwrap();
                    if target_transform
                        .translation
                        .distance(entity_transform.translation)
                        < range
                    {
                        TaskState::Running
                    } else {
                        TaskState::Failure
                    }
                }
                None => TaskState::Failure,
            }
        };
        let task = TaskImpl::new(checker)
            // Task inserts some components to the entity while running.
            .insert_while_running(Follow {
                target: None,
                speed,
            })
            // Or run some commands on enter/exit.
            .on_enter(|_entity, mut _commands| {
                info!("Beginning to follow.");
            });
        Arc::new(Self {
            task: Arc::new(task),
        })
    }
}

// Entities in the `Follow` task move toward the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
struct Follow {
    pub target: Option<Entity>,
    pub speed: f32,
}
impl Default for Follow {
    fn default() -> Self {
        Self {
            target: None,
            speed: 5.0,
        }
    }
}

// Let's define some real behavior for entities in the follow task.
fn follow(
    mut transforms: Query<&mut Transform>,
    follows: Query<(Entity, &Follow)>,
    time: Res<Time>,
) {
    for (entity, follow) in &follows {
        if let Some(follow_target) = follow.target {
            // Get the positions of the follower and target
            let target_translation = transforms.get(follow_target).unwrap().translation;
            let follow_transform = &mut transforms.get_mut(entity).unwrap();
            let follow_translation = follow_transform.translation;

            // Find the direction from the follower to the target and go that way
            follow_transform.translation += (target_translation - follow_translation)
                .normalize_or_zero()
                * follow.speed
                * time.delta_seconds();
        }
    }
}

// The code after this comment is not related to `bevior_tree`. It's just player movement.

#[derive(Component)]
pub enum Player {
    Wasd,
    ArrowKeys,
}

const PLAYER_SPEED: f32 = 200.;

fn move_player(
    mut q_players: Query<(&mut Transform, &Player)>,
    keys: Res<Input<KeyCode>>,
    time: Res<Time>,
) {
    for (mut transform, player) in q_players.iter_mut() {
        let mut controls: [KeyCode; 4] =
            [KeyCode::Right, KeyCode::Left, KeyCode::Up, KeyCode::Down];
        match player {
            Player::Wasd => controls = [KeyCode::D, KeyCode::A, KeyCode::W, KeyCode::S],
            Player::ArrowKeys => (),
        }
        transform.translation += Vec3::new(
            (keys.pressed(controls[0]) as i32 - keys.pressed(controls[1]) as i32) as f32,
            (keys.pressed(controls[2]) as i32 - keys.pressed(controls[3]) as i32) as f32,
            0.,
        )
        .normalize_or_zero()
            * PLAYER_SPEED
            * time.delta_seconds();
    }
}
