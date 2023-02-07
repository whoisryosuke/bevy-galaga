use std::time::Duration;

use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{
        collide_aabb::collide, Material2d, Material2dPlugin, MaterialMesh2dBundle, Mesh2dHandle,
    },
    time::FixedTimestep,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(Material2dPlugin::<CustomMaterial>::default())
        .insert_resource(ProjectileTimer(Timer::from_seconds(
            PROJECTILE_TIME_LIMIT,
            TimerMode::Once,
        )))
        .add_startup_system(setup_game)
        .add_system(update_material_time)
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

const PLAYER_SIZE: Vec3 = Vec3::new(15.0, 16.0, 0.0);
const PLAYER_SPEED: f32 = 400.0;
const PLAYER_STARTING_POSITION: Vec3 = Vec3::new(0.0, -300.0, 1.0);
const ENEMY_STARTING_POSITION: Vec3 = Vec3::new(0.0, 20.0, 1.0);
const PROJECTILE_SIZE: Vec3 = Vec3::splat(3.0);
const PROJECTILE_SPEED: f32 = 400.0;
const ENEMY_PROJECTILE_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);
const PLAYER_PROJECTILE_DIRECTION: Vec2 = Vec2::new(0.5, 0.5);

fn setup_game(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CustomMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Background
    commands.spawn(MaterialMesh2dBundle {
        // mesh: meshes.add(shape::Plane { size: 3.0 }.into()).into(),
        mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
        transform: Transform::default().with_scale(Vec3::new(
            1300.0,
            SCREEN_EDGE_VERTICAL * 2.0,
            0.0,
        )),
        // material: materials.add(ColorMaterial::from(Color::TURQUOISE)),
        material: materials.add(CustomMaterial {
            color: Color::BLUE,
            color_texture: Some(asset_server.load("textures/space/space.png")),
            tile: 1.0,
            time: 0.0,
        }),
        ..default()
    });

    // Spawn Player in initial position
    commands.spawn((
        MaterialMesh2dBundle {
            // mesh: meshes.add(shape::Plane { size: 3.0 }.into()).into(),
            mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
            transform: Transform {
                translation: PLAYER_STARTING_POSITION,
                scale: PLAYER_SIZE,
                ..default()
            },
            material: materials.add(CustomMaterial {
                color: Color::BLUE,
                color_texture: Some(asset_server.load("sprites/player_default.png")),
                tile: 0.0,
                time: 0.0,
            }),
            ..default()
        },
        Player,
        Collider,
    ));

    // Spawn enemies
    commands.spawn((
        MaterialMesh2dBundle {
            // mesh: meshes.add(shape::Plane { size: 3.0 }.into()).into(),
            mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
            transform: Transform {
                translation: ENEMY_STARTING_POSITION,
                scale: PLAYER_SIZE,
                ..default()
            },
            material: materials.add(CustomMaterial {
                color: Color::BLUE,
                color_texture: Some(asset_server.load("sprites/enemy_green_bug.png")),
                tile: 0.0,
                time: 0.0,
            }),
            ..default()
        },
        Enemy,
        Collider,
    ));
}

impl Material2d for CustomMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/custom_material.wgsl".into()
    }
}

// Background shader material
#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "f690fdae-d598-45ab-8225-97e2a3f056e0"]
pub struct CustomMaterial {
    #[uniform(0)]
    color: Color,
    // Should we tile this material? 1 = true
    #[uniform(0)]
    tile: f32,
    #[uniform(0)]
    time: f32,
    #[texture(1)]
    #[sampler(2)]
    color_texture: Option<Handle<Image>>,
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
    mut materials: ResMut<Assets<CustomMaterial>>,
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&Transform, With<Player>>,
    asset_server: Res<AssetServer>,
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
                    // mesh: meshes.add(shape::Plane { size: 3.0 }.into()).into(),
                    mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
                    transform: Transform {
                        translation: player_transform.translation,
                        scale: PROJECTILE_SIZE,
                        ..default()
                    },
                    material: materials.add(CustomMaterial {
                        color: Color::BLUE,
                        color_texture: Some(asset_server.load("sprites/player_projectile.png")),
                        tile: 0.0,
                        time: 0.0,
                    }),
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

fn update_material_time(time: Res<Time>, mut materials: ResMut<Assets<CustomMaterial>>) {
    materials.iter_mut().for_each(|material| {
        material.1.time = time.elapsed_seconds();
    });
}
