use std::time::Duration;

use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::render_resource::{AsBindGroup, ShaderRef},
    sprite::{
        collide_aabb::collide, Material2d, Material2dPlugin, MaterialMesh2dBundle, Mesh2dHandle,
    },
    text,
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
        .insert_resource(IntroTimer(Timer::from_seconds(
            INTRO_TIME_LIMIT,
            TimerMode::Once,
        )))
        .add_startup_system(setup_game)
        .add_system(update_material_time)
        .insert_resource(PlayerScore { score: 0 })
        .insert_resource(GameState {
            started: false,
            paused: false,
            intro: false,
            level: 1,
        })
        .add_event::<GameStartEvent>()
        .add_event::<EnemyDeathEvent>()
        .add_event::<ProjectileEvent>()
        .add_event::<NewLevelEvent>()
        .add_system_set(
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(check_for_collisions)
                .with_system(move_player.before(check_for_collisions))
                .with_system(move_projectiles.before(check_for_collisions))
                .with_system(destroy_projectiles.before(check_for_collisions))
                .with_system(play_projectile_sound.before(check_for_collisions))
                .with_system(update_player_score.before(play_enemy_death_sound))
                .with_system(play_enemy_death_sound.before(check_for_collisions))
                .with_system(shoot_projectile.before(check_for_collisions)),
        )
        .add_system(start_game)
        .add_system(pause_game)
        .add_system(play_intro)
        .add_system(display_start_screen)
        .add_system(spawn_enemies)
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

// Events
// Enemy Death
#[derive(Default)]
struct EnemyDeathEvent(usize);

// Projectile has been fired
#[derive(Default)]
struct ProjectileEvent;

// Game has started. This usually triggers intro sequence.
#[derive(Default)]
struct GameStartEvent;

// Player has started a new level. The level is the first param.
#[derive(Default)]
struct NewLevelEvent(usize);

// Sounds
#[derive(Resource)]
struct EnemyDeathSound(Handle<AudioSource>);
#[derive(Resource)]
struct ProjectileSound(Handle<AudioSource>);
#[derive(Resource)]
struct GameIntroSound(Handle<AudioSource>);

// Resources
// The players current score
#[derive(Resource)]
struct PlayerScore {
    score: usize,
}
// The players current score
#[derive(Resource)]
struct GameState {
    // Has game started? (aka user presses "start")
    started: bool,
    // Is game paused? Only relevant is game is started
    paused: bool,
    // Are we playing game intro? Occurs after initial game start.
    intro: bool,
    // The level number (1-99+)
    level: usize,
}

#[derive(Resource)]
struct GameFonts {
    body: Handle<Font>,
}

#[derive(Resource)]
struct Textures {
    enemy_green_bug: Handle<Image>,
}

// Timer used to track playback of intro
#[derive(Resource)]
struct IntroTimer(Timer);

// UI
// The player's score (should be alongside a TextBundle)
#[derive(Component)]
struct PlayerScoreText;

#[derive(Component)]
struct HighScoreText;

#[derive(Component)]
struct PressStartText;

// Defines the amount of time that should elapse between each physics step
// in this case, 60fps
const TIME_STEP: f32 = 1.0 / 60.0;
const SCREEN_WIDTH_DEFAULT: f32 = 1300.0;
const SCREEN_EDGE_VERTICAL: f32 = 350.0;
const PROJECTILE_TIME_LIMIT: f32 = 0.1;
const INTRO_TIME_LIMIT: f32 = 6.0; // seconds

// We size everything to the pixel size
const PLAYER_SIZE: Vec3 = Vec3::new(15.0, 16.0, 0.0);
// Then we scale it as needed to match the resolution.
// Hardcoded for now, but could be responsive based on window size.
const SIZE_SCALE: f32 = 2.0;
const PLAYER_SPEED: f32 = 400.0;
const PLAYER_STARTING_POSITION: Vec3 = Vec3::new(0.0, -300.0, 1.0);

// Projectiles
const PROJECTILE_SIZE: Vec3 = Vec3::new(3.0, 6.0, 0.0);
const PROJECTILE_SPEED: f32 = 400.0;
const ENEMY_PROJECTILE_DIRECTION: Vec2 = Vec2::new(0.5, -0.5);
const PLAYER_PROJECTILE_DIRECTION: Vec2 = Vec2::new(0.5, 0.5);

// Enemies
const ENEMY_STARTING_POSITION: Vec3 = Vec3::new(200.0, 20.0, 1.0);
const ENEMY_COUNT: usize = 20;
const ENEMY_GAP: f32 = 20.0;

// UI
const UI_FONT_MEDIUM: f32 = 32.0;
const UI_COLOR_RED: Color = Color::rgb(0.8, 0.0, 0.0);
const UI_COLOR_WHITE: Color = Color::rgb(0.95, 0.95, 0.95);
const UI_PADDING_CENTER_TOP: Val = Val::Px(16.0);
// We take the screen width and halve it to find center - then subtract a little more to accomodate for text size
// Ideally we should make the flex 100% width and let it center using align properties, but I couldn't get that working 🤷‍♂️
const UI_PADDING_CENTER_LEFT: Val = Val::Px(SCREEN_WIDTH_DEFAULT / 2.0 - 30.0);
const UI_START_PADDING_LEFT: Val = Val::Px(SCREEN_WIDTH_DEFAULT / 2.0 - SCREEN_WIDTH_DEFAULT / 8.0);

fn setup_game(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CustomMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Camera
    commands.spawn(Camera2dBundle::default());

    // Load sound effects
    let enemy_death_sound = asset_server.load("sounds/enemy-death.mp3");
    commands.insert_resource(EnemyDeathSound(enemy_death_sound));
    let projectile_sound = asset_server.load("sounds/projectile.mp3");
    commands.insert_resource(ProjectileSound(projectile_sound));
    let game_intro_sound = asset_server.load("sounds/intro.mp3");
    commands.insert_resource(GameIntroSound(game_intro_sound));

    // Background
    commands.spawn(MaterialMesh2dBundle {
        // mesh: meshes.add(shape::Plane { size: 3.0 }.into()).into(),
        mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
        transform: Transform::default().with_scale(Vec3::new(
            SCREEN_WIDTH_DEFAULT,
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

    // Add fonts to system
    let game_fonts = GameFonts {
        body: asset_server.load("fonts/VT323-Regular.ttf"),
    };

    // Add textures to system
    let textures = Textures {
        enemy_green_bug: asset_server.load("sprites/enemy_green_bug.png"),
    };
    commands.insert_resource(textures);

    // UI Elements
    // High Score
    commands.spawn((
        TextBundle::from_sections([
            TextSection::new(
                "HIGH SCORE\n",
                TextStyle {
                    font: game_fonts.body.clone(),
                    font_size: UI_FONT_MEDIUM,
                    color: UI_COLOR_RED,
                },
            ),
            TextSection::new(
                "20000",
                TextStyle {
                    font: game_fonts.body.clone(),
                    font_size: UI_FONT_MEDIUM,
                    color: UI_COLOR_WHITE,
                },
            ),
        ])
        .with_text_alignment(TextAlignment::TOP_CENTER)
        .with_style(Style {
            // flex_direction: FlexDirection::Row,
            // align_content: AlignContent::Center,
            // align_items: AlignItems::Center,
            // align_self: AlignSelf::Center,
            position_type: PositionType::Absolute,
            flex_wrap: FlexWrap::Wrap,
            // size: Size {
            //     width: Val::Px(SCREEN_WIDTH_DEFAULT),
            //     height: Val::Px(200.0),
            // },
            position: UiRect {
                top: UI_PADDING_CENTER_TOP,
                left: UI_PADDING_CENTER_LEFT,
                // top: Val::Px(0.0),
                // left: Val::Px(0.0),
                ..default()
            },
            ..default()
        }),
        HighScoreText,
    ));
    // Player Score
    commands.spawn((
        TextBundle::from_sections([
            TextSection::new(
                "1UP\n",
                TextStyle {
                    font: asset_server.load("fonts/VT323-Regular.ttf"),
                    font_size: UI_FONT_MEDIUM,
                    color: UI_COLOR_RED,
                },
            ),
            TextSection::from_style(TextStyle {
                font: asset_server.load("fonts/VT323-Regular.ttf"),
                font_size: UI_FONT_MEDIUM,
                color: UI_COLOR_WHITE,
            }),
        ])
        .with_style(Style {
            position_type: PositionType::Absolute,
            position: UiRect {
                top: UI_PADDING_CENTER_TOP,
                left: UI_PADDING_CENTER_TOP,
                ..default()
            },
            ..default()
        }),
        PlayerScoreText,
    ));

    // Now we can insert fonts as a resource after the UI has used it
    commands.insert_resource(game_fonts);

    // Spawn Player in initial position
    commands.spawn((
        MaterialMesh2dBundle {
            // mesh: meshes.add(shape::Plane { size: 3.0 }.into()).into(),
            mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
            transform: Transform {
                translation: PLAYER_STARTING_POSITION,
                scale: PLAYER_SIZE * SIZE_SCALE,
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
    game_state: Res<GameState>,
) {
    if game_state.started && !game_state.paused && !game_state.intro {
        let mut player_transform = query.single_mut();
        let mut direction = 0.0;

        if keyboard_input.pressed(KeyCode::Left) {
            direction -= 1.0;
        }

        if keyboard_input.pressed(KeyCode::Right) {
            direction += 1.0;
        }

        // Calculate the new horizontal player position based on player input
        let new_player_position =
            player_transform.translation.x + direction * PLAYER_SPEED * TIME_STEP;
        // TODO: make sure player doesn't exceed bounds of game area

        player_transform.translation.x = new_player_position;
    }
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
    mut projectile_events: EventWriter<ProjectileEvent>,
    game_state: Res<GameState>,
) {
    if game_state.started && !game_state.paused && !game_state.intro {
        let player_transform = query.single_mut();

        if keyboard_input.pressed(KeyCode::Space) {
            // Check if player is allowed to shoot based on internal timer
            // We have to "tick" the timer to update it with the latest time
            if projectile_timer.0.tick(time.delta()).finished() {
                // Reset the timer
                projectile_timer.0.reset();

                // Fire off a ProjectileEvent to notify other systems
                projectile_events.send_default();

                // Spawn projectile
                commands.spawn((
                    MaterialMesh2dBundle {
                        // mesh: meshes.add(shape::Plane { size: 3.0 }.into()).into(),
                        mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
                        transform: Transform {
                            translation: player_transform.translation,
                            scale: PROJECTILE_SIZE * SIZE_SCALE,
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
    mut death_events: EventWriter<EnemyDeathEvent>,
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
                    // Fire off a EnemyDeathEvent to notify other systems
                    // death_events.send_default();
                    death_events.send(EnemyDeathEvent(100));

                    // Enemy is destroyed
                    commands.entity(collider_entity).despawn();

                    // Projectile disappears too? Prevents "cutting through" a line of enemies all at once
                    commands.entity(projectile_entity).despawn();
                }
            }
        }
    }
}

fn play_enemy_death_sound(
    death_events: EventReader<EnemyDeathEvent>,
    audio: Res<Audio>,
    sound: Res<EnemyDeathSound>,
) {
    // Check for events
    if !death_events.is_empty() {
        // Clear all events this frame
        death_events.clear();

        audio.play(sound.0.clone());
    }
}

fn play_projectile_sound(
    projectile_events: EventReader<ProjectileEvent>,
    audio: Res<Audio>,
    sound: Res<ProjectileSound>,
) {
    // Check for events
    if !projectile_events.is_empty() {
        // Clear all events this frame
        projectile_events.clear();
        println!("[AUDIO] Playing projectile sound!");

        audio.play(sound.0.clone());
    }
}

fn update_material_time(time: Res<Time>, mut materials: ResMut<Assets<CustomMaterial>>) {
    materials.iter_mut().for_each(|material| {
        material.1.time = time.elapsed_seconds();
    });
}

fn update_player_score(
    mut player_score: ResMut<PlayerScore>,
    mut enemy_death_events: EventReader<EnemyDeathEvent>,
    mut query: Query<&mut Text, With<PlayerScoreText>>,
) {
    // Check for events
    if !enemy_death_events.is_empty() {
        println!("[UI] Updating player score");

        enemy_death_events.iter().for_each(|event| {
            // let EnemyDeathEvent(points) = event;
            // dbg!(&points);
            // dbg!(&event.0);
            player_score.score += &event.0;
        });

        for mut text in &mut query {
            text.sections[1].value = player_score.score.to_string();
        }
    }
}

fn start_game(
    mut game_state: ResMut<GameState>,
    keyboard_input: Res<Input<KeyCode>>,
    mut start_events: EventWriter<GameStartEvent>,
) {
    // If game hasn't started, detect space/return key to start game
    if !game_state.started {
        if keyboard_input.pressed(KeyCode::Space) | keyboard_input.pressed(KeyCode::Return) {
            println!("[INPUT] Game Started");
            game_state.started = true;

            // Let other systems know we started (like intro sequence)
            start_events.send_default();
        }
    }
}

fn pause_game(mut game_state: ResMut<GameState>, keyboard_input: Res<Input<KeyCode>>) {
    // If game has started, check for P key to pause game
    if game_state.started {
        if keyboard_input.pressed(KeyCode::P) {
            game_state.paused = !game_state.paused;
        }
    }
}

fn play_intro(
    time: Res<Time>,
    mut game_state: ResMut<GameState>,
    audio: Res<Audio>,
    sound: Res<GameIntroSound>,
    start_events: EventReader<GameStartEvent>,
    mut intro_timer: ResMut<IntroTimer>,
    mut level_events: EventWriter<NewLevelEvent>,
) {
    // Did the game just start? Play the intro music and reset timer.
    if !start_events.is_empty() {
        start_events.clear();

        // Let the app know we're in an intro sequence - doesn't have to be event
        game_state.intro = true;

        // Play the intro song
        audio.play(sound.0.clone());

        intro_timer.0.reset();
    }

    // If the intro is playing, we increment it's timer to know if it's done or not
    if game_state.intro && intro_timer.0.tick(time.delta()).just_finished() {
        game_state.intro = false;

        level_events.send(NewLevelEvent(1));
    }
}

fn display_start_screen(
    mut commands: Commands,
    game_fonts: Res<GameFonts>,
    game_state: Res<GameState>,
    query: Query<Entity, With<PressStartText>>,
) {
    let mut start_screen_exists = false;
    for text_obj in &query {
        // commands.entity(text_obj).id()
        start_screen_exists = true;
        break;
    }

    // Game hasn't started and we haven't spawned UI yet
    if !game_state.started && !start_screen_exists {
        // Display UI for Start Screen
        commands.spawn((
            TextBundle::from_sections([TextSection::new(
                "Press Spacebar/Return to Start \n".to_uppercase(),
                TextStyle {
                    font: game_fonts.body.clone(),
                    font_size: UI_FONT_MEDIUM,
                    color: UI_COLOR_RED,
                },
            )])
            .with_text_alignment(TextAlignment::TOP_CENTER)
            .with_style(Style {
                position_type: PositionType::Absolute,
                position: UiRect {
                    top: Val::Px(SCREEN_EDGE_VERTICAL),
                    left: UI_START_PADDING_LEFT,
                    // left: Val::Px(0.0),
                    ..default()
                },
                ..default()
            }),
            PressStartText,
        ));
    }

    // Game started! Remove any UI.
    if game_state.started && start_screen_exists {
        for text_obj in &query {
            commands.entity(text_obj).despawn();
        }
    }
}

fn spawn_enemies(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CustomMaterial>>,
    textures: Res<Textures>,
    mut level_events: EventReader<NewLevelEvent>,
    mut game_state: ResMut<GameState>,
) {
    // Check for events
    if !level_events.is_empty() {
        // We grab the level number from the NewLevelEvent
        level_events.iter().for_each(|level| {
            game_state.level = level.0;
        });

        // Clear all events this frame
        level_events.clear();

        for enemy_id in 0..ENEMY_COUNT {
            dbg!(enemy_id);

            // Spawn enemies
            commands.spawn((
                MaterialMesh2dBundle {
                    mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
                    transform: Transform {
                        translation: ENEMY_STARTING_POSITION
                            - Vec3::new(enemy_id as f32 * ENEMY_GAP * SIZE_SCALE, 0.0, 0.0),
                        scale: PLAYER_SIZE * SIZE_SCALE,
                        ..default()
                    },
                    material: materials.add(CustomMaterial {
                        color: Color::BLUE,
                        color_texture: Some(textures.enemy_green_bug.clone()),
                        tile: 0.0,
                        time: 0.0,
                    }),
                    ..default()
                },
                Enemy,
                Collider,
            ));
        }
    }
}
