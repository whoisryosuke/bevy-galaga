use std::time::Duration;

use bevy::{
    prelude::*,
    sprite::{collide_aabb::collide, MaterialMesh2dBundle},
    time::FixedTimestep,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(ProjectileTimer(Timer::from_seconds(
            PROJECTILE_TIME_LIMIT,
            TimerMode::Once,
        )))
        .add_startup_system(setup_game)
        .add_system_set(
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(check_for_collisions)
                .with_system(move_player.before(check_for_collisions))
                .with_system(move_projectiles.before(check_for_collisions))
                .with_system(destroy_projectiles.before(check_for_collisions))
                .with_system(shoot_projectile.before(check_for_collisions)),
        )
        .add_system(bevy::window::close_on_esc)
        .run();
}

// The Player object
#[derive(Component)]
struct Player;

// The Enemy object
#[derive(Component)]
struct Enemy;

// The projectile spawned by Player firing weapon
#[derive(Component)]
struct Projectile;

// Timer used to limit player shooting every frame per second
#[derive(Resource)]
struct ProjectileTimer(Timer);

// The speed of an object
#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

// Signifies an object is collidable
#[derive(Component)]
struct Collider;

// Defines the amount of time that should elapse between each physics step
// in this case, 60fps
const TIME_STEP: f32 = 1.0 / 60.0;
const SCREEN_EDGE_VERTICAL: f32 = 350.0;
const PROJECTILE_TIME_LIMIT: f32 = 0.1;

const PLAYER_SIZE: Vec3 = Vec3::new(2.0, 2.0, 0.0);
const PLAYER_SPEED: f32 = 400.0;
const PLAYER_STARTING_POSITION: Vec3 = Vec3::new(0.0, -300.0, 0.0);
const PROJECTILE_STARTING_POSITION: Vec3 = Vec3::new(0.0, 20.0, 0.0);
const PROJECTILE_SIZE: Vec3 = Vec3::new(10.0, 10.0, 0.0);
const PROJECTILE_SPEED: f32 = 400.0;
const ENEMY_PROJECTILE_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);
const PLAYER_PROJECTILE_DIRECTION: Vec2 = Vec2::new(0.5, 0.5);

const PLAYER_COLOR: Color = Color::rgb(0.3, 0.3, 0.7);
const PROJECTILE_COLOR: Color = Color::rgb(0.7, 0.87, 0.7);

fn setup_game(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Spawn Player in initial position
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("sprites/player_default.png"),
            transform: Transform {
                translation: PLAYER_STARTING_POSITION,
                scale: PLAYER_SIZE,
                ..default()
            },
            sprite: Sprite {
                // color: PLAYER_COLOR,
                ..default()
            },
            ..default()
        },
        Player,
        Collider,
    ));

    // Spawn enemies
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::default().into()).into(),
            material: materials.add(ColorMaterial::from(PROJECTILE_COLOR)),
            transform: Transform::from_translation(PROJECTILE_STARTING_POSITION)
                .with_scale(PROJECTILE_SIZE * Vec3::new(2.0, 2.0, 2.0)),
            ..default()
        },
        Enemy,
        Collider,
    ));
}

fn move_player(
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let mut player_transform = query.single_mut();
    let mut direction = 0.0;

    if keyboard_input.pressed(KeyCode::Left) {
        direction -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::Right) {
        direction += 1.0;
    }

    // Calculate the new horizontal player position based on player input
    let new_player_position = player_transform.translation.x + direction * PLAYER_SPEED * TIME_STEP;
    // TODO: make sure player doesn't exceed bounds of game area

    player_transform.translation.x = new_player_position;
}

fn shoot_projectile(
    time: Res<Time>,
    mut projectile_timer: ResMut<ProjectileTimer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&Transform, With<Player>>,
) {
    let player_transform = query.single_mut();

    if keyboard_input.pressed(KeyCode::Space) {
        // Check if player is allowed to shoot based on internal timer
        // We have to "tick" the timer to update it with the latest time
        if projectile_timer.0.tick(time.delta()).finished() {
            // Reset the timer
            projectile_timer.0.reset();

            // Spawn projectile
            commands.spawn((
                MaterialMesh2dBundle {
                    mesh: meshes.add(shape::Circle::default().into()).into(),
                    material: materials.add(ColorMaterial::from(PROJECTILE_COLOR)),
                    transform: Transform::from_translation(player_transform.translation)
                        .with_scale(PROJECTILE_SIZE),
                    ..default()
                },
                Projectile,
                Velocity(PLAYER_PROJECTILE_DIRECTION.normalize() * PROJECTILE_SPEED),
            ));
        }
    }
}

fn move_projectiles(mut query: Query<(&mut Transform, &Velocity), With<Projectile>>) {
    for (mut collider_transform, velocity) in &mut query {
        // Calculate the new horizontal player position based on player input
        let new_projectile_position = collider_transform.translation.y + velocity.y * TIME_STEP;
        // TODO: make sure player doesn't exceed bounds of game area

        collider_transform.translation.y = new_projectile_position;
    }
}

fn destroy_projectiles(
    mut commands: Commands,
    query: Query<(Entity, &Transform), With<Projectile>>,
) {
    for (collider_entity, collider_transform) in &query {
        // Check if projectile has passed top or bottom of screen
        if collider_transform.translation.y > SCREEN_EDGE_VERTICAL
            || collider_transform.translation.y < -SCREEN_EDGE_VERTICAL
        {
            commands.entity(collider_entity).despawn();
        }
    }
}

fn check_for_collisions(
    mut commands: Commands,
    projectiles_query: Query<(Entity, &Transform), With<Projectile>>,
    collider_query: Query<(Entity, &Transform, Option<&Enemy>), With<Collider>>,
) {
    // Loop through all the projectiles on screen
    for (projectile_entity, projectile_transform) in &projectiles_query {
        // Loop through all collidable elements on the screen
        // TODO: Figure out how to flatten this - 2 for loops no bueno
        for (collider_entity, collider_transform, enemy_check) in &collider_query {
            let collision = collide(
                projectile_transform.translation,
                projectile_transform.scale.truncate(),
                collider_transform.translation,
                collider_transform.scale.truncate(),
            );

            if let Some(collision) = collision {
                // If it's an enemy, destroy!
                if enemy_check.is_some() {
                    println!("Collided!");

                    // Enemy is destroyed
                    commands.entity(collider_entity).despawn();

                    // Projectile disappears too? Prevents "cutting through" a line of enemies all at once
                    commands.entity(projectile_entity).despawn();
                }
            }
        }
    }
}
