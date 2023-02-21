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
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugin(Material2dPlugin::<CustomMaterial>::default())
        .insert_resource(ProjectileTimer(Timer::from_seconds(
            PROJECTILE_TIME_LIMIT,
            TimerMode::Once,
        )))
        .insert_resource(IntroTimer(Timer::from_seconds(
            INTRO_TIME_LIMIT,
            TimerMode::Once,
        )))
        .insert_resource(EnemySpawnTimer(Timer::from_seconds(
            ENEMY_TIME,
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
        .insert_resource(GameSettingsState { volume: 0.1 })
        .insert_resource(EnemySpawnState {
            current_group: 0,
            groups: vec![],
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
                .with_system(animate_explosion)
                .with_system(shoot_projectile.before(check_for_collisions)),
        )
        .add_system(start_game)
        .add_system(pause_game)
        .add_system(play_intro)
        .add_system(display_start_screen)
        .add_system(spawn_enemies)
        .add_system(spawn_enemy_group)
        .add_system(intro_enemy_group_dance)
        .add_system(bevy::window::close_on_esc)
        .run();
}

// The Player object
#[derive(Component)]
struct Player;

// The Enemy object
#[derive(Component)]
struct Enemy;

// The EnemyGroup object.
// First `usize` = What group ID the enemy is in.
// Second `usize` = Enemy ID
#[derive(Component)]
struct EnemyGroupComponent(usize, usize);

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
// Global game state (level management, un/paused, etc)
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

// The players settings
#[derive(Resource)]
struct GameSettingsState {
    // Volume of game (1 = full volume)
    volume: f32,
}

// Galaga spawns multiple enemies at a time in groups,
// so we use this to keep track of their "intro dance"
#[derive(Resource)]
struct EnemySpawnState {
    // Index of current enemy group
    current_group: usize,
    // Enemy groups. Each group is a vector of different enemies (e.g. blue vs red bugs)
    groups: Vec<EnemyGroup>,
}

// All the enemy types in game
enum EnemyTypes {
    GreenBug,
}

struct EnemyData {
    enemy_type: EnemyTypes,
    // Where enemy ends up
    end_position: Vec3,
}

struct EnemyGroup {
    group: Vec<EnemyData>,
    finished: bool,
}

// Timer used to track time between spawning new enemy groups
#[derive(Resource)]
struct EnemySpawnTimer(Timer);

#[derive(Resource)]
struct GameFonts {
    body: Handle<Font>,
}

#[derive(Resource)]
struct Textures {
    enemy_green_bug: Handle<Image>,
    explosion_enemy: Handle<Image>,
}

// Timer used to track playback of intro
#[derive(Resource)]
struct IntroTimer(Timer);

// Timer used to track playback of animations
#[derive(Component)]
struct AnimationTimer(Timer);
// The current frame of animation
#[derive(Component)]
struct AnimationFrame(usize);

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
const SCREEN_EDGE_VERTICAL: f32 = 360.0;
const PROJECTILE_TIME_LIMIT: f32 = 0.3;
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
// This is the position of the enemy that's hiding beyond top of screen
const ENEMY_INTRO_POSITION: Vec3 = Vec3::new(0.0, SCREEN_EDGE_VERTICAL + 20.0, 1.0);
// Position of the top "line" the enemies form as a grid.
const ENEMY_LINE_POSITION: Vec3 = Vec3::new(-400.0, 20.0, 1.0);
const ENEMY_COUNT: usize = 20;
const ENEMY_GAP: f32 = 20.0;
const ENEMY_TIME: f32 = 3.0; // seconds

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
        explosion_enemy: asset_server.load("sprites/explosion_enemy.png"),
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
impl Default for CustomMaterial {
    fn default() -> Self {
        CustomMaterial {
            color: Color::BLUE,
            tile: 0.0,
            time: 0.0,
            color_texture: None,
        }
    }
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
    textures: Res<Textures>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
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

                    // Spawn explosion
                    let texture_atlas = TextureAtlas::from_grid(
                        textures.explosion_enemy.clone(),
                        Vec2::new(30.0, 32.0),
                        4,
                        1,
                        None,
                        None,
                    );
                    let texture_atlas_handle = texture_atlases.add(texture_atlas);

                    let mut position = Transform::from_scale(Vec3::splat(SIZE_SCALE));
                    position.translation = collider_transform.translation.clone();

                    commands.spawn((
                        SpriteSheetBundle {
                            texture_atlas: texture_atlas_handle,
                            transform: position,
                            ..default()
                        },
                        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
                        AnimationFrame(0),
                    ));

                    // Enemy is destroyed
                    commands.entity(collider_entity).despawn();

                    // Projectile disappears too? Prevents "cutting through" a line of enemies all at once
                    commands.entity(projectile_entity).despawn();
                }
            }
        }
    }
}

// Animate any explosions in scene frame by frame and despawn after last one
fn animate_explosion(
    mut commands: Commands,
    time: Res<Time>,
    texture_atlases: Res<Assets<TextureAtlas>>,
    mut query: Query<(
        Entity,
        &mut AnimationTimer,
        &mut TextureAtlasSprite,
        &Handle<TextureAtlas>,
    )>,
) {
    for (entity, mut timer, mut sprite, texture_atlas_handle) in &mut query {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            let texture_atlas = texture_atlases.get(texture_atlas_handle).unwrap();

            // Check if last frame and delete explosion if so
            // Sprite Index starts from 0, so add 1 to match texture atlas length
            if texture_atlas.textures.len() == sprite.index + 1 {
                commands.entity(entity).despawn();
                return;
            }

            // Otherwise animate one more frame
            sprite.index = (sprite.index + 1) % texture_atlas.textures.len();
        }
    }
}

fn play_enemy_death_sound(
    death_events: EventReader<EnemyDeathEvent>,
    audio: Res<Audio>,
    sound: Res<EnemyDeathSound>,
    settings: Res<GameSettingsState>,
) {
    // Check for events
    if !death_events.is_empty() {
        // Clear all events this frame
        death_events.clear();

        audio.play_with_settings(
            sound.0.clone(),
            PlaybackSettings {
                volume: settings.volume,
                ..Default::default()
            },
        );
    }
}

fn play_projectile_sound(
    projectile_events: EventReader<ProjectileEvent>,
    audio: Res<Audio>,
    sound: Res<ProjectileSound>,
    settings: Res<GameSettingsState>,
) {
    // Check for events
    if !projectile_events.is_empty() {
        // Clear all events this frame
        projectile_events.clear();
        println!("[AUDIO] Playing projectile sound!");

        audio.play_with_settings(
            sound.0.clone(),
            PlaybackSettings {
                volume: settings.volume,
                ..Default::default()
            },
        );
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
    settings: Res<GameSettingsState>,
) {
    // Did the game just start? Play the intro music and reset timer.
    if !start_events.is_empty() {
        start_events.clear();

        // Let the app know we're in an intro sequence - doesn't have to be event
        game_state.intro = true;

        // Play the intro song
        audio.play_with_settings(
            sound.0.clone(),
            PlaybackSettings {
                volume: settings.volume,
                ..Default::default()
            },
        );

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
    mut level_events: EventReader<NewLevelEvent>,
    mut game_state: ResMut<GameState>,
    mut enemy_spawn_state: ResMut<EnemySpawnState>,
) {
    // Check for events
    if !level_events.is_empty() {
        // We grab the level number from the NewLevelEvent
        level_events.iter().for_each(|level| {
            game_state.level = level.0;
        });

        // Clear all events this frame
        level_events.clear();

        let mut new_enemy_groups: Vec<EnemyGroup> = Vec::new();
        for group_id in 0..2 {
            let mut group: Vec<EnemyData> = Vec::new();
            for enemy_id in 0..ENEMY_COUNT {
                group.push(EnemyData {
                    enemy_type: EnemyTypes::GreenBug,
                    end_position: ENEMY_LINE_POSITION
                        + Vec3 {
                            x: enemy_id as f32 * ENEMY_GAP,
                            y: 0.0,
                            z: 0.0,
                        },
                });
            }

            let new_group = EnemyGroup {
                group,
                finished: false,
            };
            new_enemy_groups.push(new_group);
        }

        enemy_spawn_state.current_group = 0;
        enemy_spawn_state.groups = new_enemy_groups;
    }
}

// After we define enemy groups, each group spawns and flys into screen (making circles and whatnot)
fn spawn_enemy_group(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CustomMaterial>>,
    textures: Res<Textures>,
    mut enemy_spawn_state: ResMut<EnemySpawnState>,
    mut enemy_timer: ResMut<EnemySpawnTimer>,
    time: Res<Time>,
) {
    // Check if we're on the last group - stop if so
    if enemy_spawn_state.current_group == enemy_spawn_state.groups.len() {
        return;
    }

    // Enemy timer finished? Spawn enemies and reset timer

    if enemy_timer.0.tick(time.delta()).finished() {
        let current_group = &enemy_spawn_state.groups[enemy_spawn_state.current_group];

        let mut enemy_id = 0;
        for enemy in &current_group.group {
            // Spawn enemies
            commands.spawn((
                MaterialMesh2dBundle {
                    mesh: meshes.add(Mesh::from(shape::Quad::default())).into(),
                    transform: Transform {
                        translation: ENEMY_INTRO_POSITION
                            + Vec3::new(0.0, enemy_id as f32 * ENEMY_GAP * SIZE_SCALE, 0.0),
                        scale: PLAYER_SIZE * SIZE_SCALE,
                        ..default()
                    },
                    material: materials.add(CustomMaterial {
                        color: Color::BLUE,
                        color_texture: Some(textures.enemy_green_bug.clone()),
                        ..Default::default()
                    }),
                    ..default()
                },
                Enemy,
                Collider,
                EnemyGroupComponent(enemy_spawn_state.current_group, enemy_id),
            ));

            enemy_id += 1;
        }

        // Reset the enemy spawn timer
        enemy_timer.0.reset();

        // Increment to the next group
        enemy_spawn_state.current_group += 1;

        println!("[ENEMY] Spawning GROUP");
    }
}

fn intro_enemy_group_dance(
    mut query: Query<(&mut Transform, &EnemyGroupComponent), With<Enemy>>,
    mut enemy_spawn_state: ResMut<EnemySpawnState>,
    time: Res<Time>,
) {
    // Loop through all enemies
    for (mut enemy_position, enemy_group_id_option) in &mut query {
        let EnemyGroupComponent(enemy_group_id, enemy_id) = enemy_group_id_option;

        // If this is the current group (or any previous that haven't finished)
        if enemy_group_id <= &enemy_spawn_state.current_group
            && !&enemy_spawn_state.groups[*enemy_group_id].finished
        {
            // Move enemy into position. We animate smoother using a "lerp" to enable "easing".
            // Enemy starts at top of screen (where they initially spawn) and travel directly to position in "line"
            // let new_projectile_position = enemy_position.translation.y - 100.0 * TIME_STEP;
            // let new_projectile_position = lerp(ENEMY_INTRO_POSITION.y, ENEMY_LINE_POSITION.y, 0.1);
            let final_y = ENEMY_LINE_POSITION.y + *enemy_group_id as f32 * ENEMY_GAP * SIZE_SCALE;
            let new_projectile_position_y = lerp(enemy_position.translation.y, final_y, 0.1);
            let final_x = ENEMY_LINE_POSITION.x + *enemy_id as f32 * ENEMY_GAP * SIZE_SCALE;
            let new_projectile_position_x = lerp(enemy_position.translation.x, final_x, 0.1);
            // @TODO: Calculate a "next" position and lerp to that instead (to get the "circular" motion)
            // @TODO: Yet animation should still and at same point eventually -- maybe second phase (return to home kinda system)

            enemy_position.translation.y = new_projectile_position_y;
            enemy_position.translation.x = new_projectile_position_x;

            // println!("enemy position: {:?}", enemy_position.translation.y);

            if enemy_position.translation.y == final_y && enemy_position.translation.x == final_x {
                enemy_spawn_state.groups[*enemy_group_id].finished = true;
            }
        }
    }
}

// Utility funcitons
fn lerp(start: f32, end: f32, amt: f32) -> f32 {
    return (1.0 - amt) * start + amt * end;
}
