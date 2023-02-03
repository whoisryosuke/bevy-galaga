use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_game)
        .run();
}

fn setup_game(mut commands: Commands) {}
