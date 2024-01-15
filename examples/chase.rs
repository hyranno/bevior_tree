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
        BehaviorTreeBundle::from_root(
            ConditionalLoop::new(
                Sequence::new(vec![
                    // Task to wait until player get near.
                    Box::new(NearTask::new(player, 300.)),
                    // Task to follow the player.
                    Box::new(FollowTask::new(player, 300., 100.)),
                ]),
                |In(_)| true
            )
        ),
    ));
}

fn get_distance(entity0: Entity, entity1: Entity, param: Query<&Transform>) -> f32 {
    param.get(entity0).unwrap().translation.truncate()
        .distance(param.get(entity1).unwrap().translation.truncate())
}



// Task to wait until player get near.
// Task trait is available for making your task, delegating core methods to TaskImpl.
struct NearTask {
    delegate: TaskBridge,
}
impl NearTask {
    pub fn new(
        target: Entity,
        range: f32,
    ) -> Self {
        let checker = move |In(entity): In<Entity>, param: Query<&Transform>| {
            let distance = get_distance(entity, target, param);
            // Check whether the target is within range. If it is, return `Success`.
            match distance <= range {
                true => TaskStatus::Complete(NodeResult::Success),
                false => TaskStatus::Running,
            } 
        };
        Self {
            delegate: TaskBridge::new(checker),
        }
    }
}
impl bevior_tree::node::Node for NearTask {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
    }
}


// Task node to follow the target.
struct FollowTask {
    delegate: TaskBridge,
}
impl FollowTask {
    pub fn new(
        target: Entity,
        range: f32,
        speed: f32,
    ) -> Self {
        let checker = move |In(entity): In<Entity>, param: Query<&Transform>| {
            let distance = get_distance(entity, target, param);
            // Return `Failure` if target is out of range.
            match distance <= range {
                true => TaskStatus::Running,
                false => TaskStatus::Complete(NodeResult::Failure),
            } 
        };
        let task = TaskBridge::new(checker)
            // Task inserts some components to the entity while running.
            .insert_while_running(Follow {target, speed})
            // Or run some commands on enter/exit.
            .on_event(TaskEvent::Enter,|_entity: In<Entity>, mut _commands: Commands| { info!("Beginning to follow."); })
        ;
        Self {
            delegate: task,
        }
    }
}
impl bevior_tree::node::Node for FollowTask {
    fn begin(&self, world: &mut World, entity: Entity) -> NodeStatus {
        self.delegate.begin(world, entity)
    }
    fn resume(&self, world: &mut World, entity: Entity, state: Box<dyn NodeState>) -> NodeStatus {
        self.delegate.resume(world, entity, state)
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
