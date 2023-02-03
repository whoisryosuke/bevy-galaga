use bevy::{prelude::*, sprite::MaterialMesh2dBundle};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_game)
        .add_system_set(
            SystemSet::new()
                // .with_system(check_for_collisions)
                // .with_system(move_player.before(check_for_collisions))
                .with_system(move_player),
        )
        .run();
}

// The Player object
#[derive(Component)]
struct Player;

// The projectile spawned by Player firing weapon
#[derive(Component)]
struct Projectile;

// The speed of an object
#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec2);

// Signifies an object is collidable
#[derive(Component)]
struct Collider;

// Defines the amount of time that should elapse between each physics step
// in this case, 60fps
const TIME_STEP: f32 = 1.0 / 60.0;

const PLAYER_SIZE: Vec3 = Vec3::new(120.0, 20.0, 0.0);
const PLAYER_SPEED: f32 = 100.0;
const PROJECTILE_STARTING_POSITION: Vec3 = Vec3::new(0.0, 20.0, 0.0);
const PROJECTILE_SIZE: Vec3 = Vec3::new(10.0, 10.0, 0.0);
const PROJECTILE_SPEED: f32 = 400.0;
const INITIAL_PROJECTILE_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);

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
            transform: Transform {
                translation: Vec3::new(0.0, -250.0, 0.0),
                scale: PLAYER_SIZE,
                ..default()
            },
            sprite: Sprite {
                color: PLAYER_COLOR,
                ..default()
            },
            ..default()
        },
        Player,
        Collider,
    ));

    // Ball
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::default().into()).into(),
            material: materials.add(ColorMaterial::from(PROJECTILE_COLOR)),
            transform: Transform::from_translation(PROJECTILE_STARTING_POSITION)
                .with_scale(PROJECTILE_SIZE),
            ..default()
        },
        Projectile,
        Velocity(INITIAL_PROJECTILE_DIRECTION.normalize() * PROJECTILE_SPEED),
    ));
}

fn move_player(
    keyboard_input: Res<Input<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let mut paddle_transform = query.single_mut();
    let mut direction = 0.0;

    if keyboard_input.pressed(KeyCode::Left) {
        direction -= 1.0;
    }

    if keyboard_input.pressed(KeyCode::Right) {
        direction += 1.0;
    }

    // Calculate the new horizontal paddle position based on player input
    let new_paddle_position = paddle_transform.translation.x + direction * PLAYER_SPEED * TIME_STEP;
    // TODO: make sure player doesn't exceed bounds of game area

    paddle_transform.translation.x = new_paddle_position;
}
