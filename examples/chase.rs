// In this game, the player moves around in 2D with the arrow keys, but if they get too close to the
// enemy, the enemy moves towards them, until the player moves back out of range.
// This example matches to the one of `seldom_state`.

use bevior_tree::prelude::*;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, BehaviorTreePlugin::default()))
        // This plugin is required for `bevior_tree`
        .add_systems(Startup, init)
        .add_systems(Update, (follow, move_player))
        .run();
}

// Setup the game
fn init(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut tree_assets: ResMut<Assets<BehaviorTreeRoot>>,
) {
    commands.spawn(Camera2d::default());

    // Simple player entity
    let player = commands
        .spawn((
            Sprite {
                image: asset_server.load("player.png"),
                ..default()
            },
            Player,
        ))
        .id();

    // The enemy
    commands.spawn((
        Sprite {
            image: asset_server.load("enemy.png"),
            ..default()
        },
        Transform::from_xyz(500., 0., 0.),
        // This behavior tree handles the enemy's behavior.
        BehaviorTree::from_node(
            ConditionalLoop::new(
                Sequence::new(vec![
                    // Task to wait until player get near.
                    Box::new(TaskBridge::new(Box::new(NearTaskDefinition { target: player, range: 300. }))),
                    // Task to follow the player.
                    Box::new(TaskBridge::new(Box::new(FollowTaskDefinition { target: player, range: 300., speed: 100. }))),
                ]),
                |In(_)| true,
            ),
            &mut tree_assets,
        ),
    ));
}

fn get_distance(entity0: Entity, entity1: Entity, param: Query<&Transform>) -> f32 {
    param
        .get(entity0)
        .unwrap()
        .translation
        .truncate()
        .distance(param.get(entity1).unwrap().translation.truncate())
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct NearTaskDefinition {
    target: Entity,
    range: f32,
}
#[cfg_attr(feature = "serde", typetag::serde)]
impl TaskDefinition for NearTaskDefinition {
    fn build_checker(&self) -> Box<dyn TaskChecker> {
        let target = self.target;
        let range = self.range;
        Box::new(IntoSystem::into_system(move |In(entity): In<Entity>, param: Query<&Transform>| {
            let distance = get_distance(entity, target, param);
            // Check whether the target is within range. If it is, return `Success`.
            match distance <= range {
                true => TaskStatus::Complete(NodeResult::Success),
                false => TaskStatus::Running,
            }
        }))
    }
    fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<dyn TaskEventListener>)> {
        vec![]
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct FollowTaskDefinition {
    target: Entity,
    range: f32,
    speed: f32,
}
#[cfg_attr(feature = "serde", typetag::serde)]
impl TaskDefinition for FollowTaskDefinition {
    fn build_checker(&self) -> Box<dyn TaskChecker> {
        let target = self.target;
        let range = self.range;
        Box::new(IntoSystem::into_system(move |In(entity): In<Entity>, param: Query<&Transform>| {
            let distance = get_distance(entity, target, param);
            // Return `Failure` if target is out of range.
            match distance <= range {
                true => TaskStatus::Running,
                false => TaskStatus::Complete(NodeResult::Failure),
            }
        }))
    }
    fn build_event_listeners(&self) -> Vec<(TaskEvent, Box<dyn TaskEventListener>)> {
        // Task inserts some components to the entity while running.
        let mut listeners = insert_while_running(Follow { target: self.target, speed: self.speed });
        listeners.push(
            // Or run some commands on enter/exit.
            (
                TaskEvent::Enter,
                Box::new(IntoSystem::into_system(move |In(_entity): In<Entity>, mut _commands: Commands| {
                    info!("Beginning to follow.");
                }))
            )
        );
        listeners
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
            * time.delta_secs();
    }
}

// The code after this comment is not related to `bevior_tree`. It's just player movement.

#[derive(Component)]
struct Player;

const PLAYER_SPEED: f32 = 200.;

fn move_player(
    mut players: Query<&mut Transform, With<Player>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    players
        .single_mut()
        .expect("Failed to get the player!")
        .translation += Vec3::new(
        (keys.pressed(KeyCode::ArrowRight) as i32 - keys.pressed(KeyCode::ArrowLeft) as i32) as f32,
        (keys.pressed(KeyCode::ArrowUp) as i32 - keys.pressed(KeyCode::ArrowDown) as i32) as f32,
        0.,
    )
    .normalize_or_zero()
        * PLAYER_SPEED
        * time.delta_secs();
}
