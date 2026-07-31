//! Cámara rápida: a qué ritmo se mira el partido, no cómo ocurre.
//!
//! La simulación es de paso fijo, así que acelerar es correr más pasos por
//! segundo de reloj de pared: los mismos ticks, en el mismo orden, con el mismo
//! resultado. Lo único que cambia es cuánto hay que esperar para verlo.

use bevy::prelude::*;
use bevy::time::Virtual;
use std::time::Duration;

/// Los multiplicadores que se ofrecen, de menor a mayor.
const SPEEDS: [f32; 7] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// Índice de la velocidad normal dentro de [`SPEEDS`].
const NORMAL: usize = 2;

/// Cuánto tiempo simulado puede avanzar un frame. El techo por defecto de Bevy
/// —0,25 s— recorta en silencio a partir de 8×, y un límite que frena sin
/// avisar se lee como que la aceleración dejó de funcionar.
const MAX_FRAME_ADVANCE: Duration = Duration::from_secs(2);

/// **.** acelera, **,** frena, **0** vuelve a tiempo real y **P** pausa.
pub struct MatchPlaybackPlugin;

impl Plugin for MatchPlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaybackSpeed>()
            .add_systems(Startup, (lift_the_frame_ceiling, spawn_speed_readout))
            .add_systems(
                Update,
                (choose_playback_speed, draw_speed_readout)
                    .chain()
                    .run_if(resource_exists::<ButtonInput<KeyCode>>),
            );
    }
}

/// Qué multiplicador se está mirando. El reloj virtual es la verdad; esto es el
/// escalón elegido, que un `f32` suelto no sabría recorrer.
#[derive(Resource, Debug, Clone, Copy)]
pub struct PlaybackSpeed {
    step: usize,
    paused: bool,
}

impl Default for PlaybackSpeed {
    fn default() -> Self {
        Self {
            step: NORMAL,
            paused: false,
        }
    }
}

impl PlaybackSpeed {
    pub fn multiplier(self) -> f32 {
        SPEEDS[self.step]
    }

    pub fn is_paused(self) -> bool {
        self.paused
    }

    fn faster(&mut self) {
        self.step = (self.step + 1).min(SPEEDS.len() - 1);
    }

    fn slower(&mut self) {
        self.step = self.step.saturating_sub(1);
    }
}

fn lift_the_frame_ceiling(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.set_max_delta(MAX_FRAME_ADVANCE);
}

fn choose_playback_speed(
    keys: Res<ButtonInput<KeyCode>>,
    mut speed: ResMut<PlaybackSpeed>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    let mut chosen = *speed;
    if keys.just_pressed(KeyCode::Period) {
        chosen.faster();
    }
    if keys.just_pressed(KeyCode::Comma) {
        chosen.slower();
    }
    if keys.just_pressed(KeyCode::Digit0) {
        chosen.step = NORMAL;
    }
    if keys.just_pressed(KeyCode::Pause) {
        chosen.paused = !chosen.paused;
    }

    if chosen.step == speed.step && chosen.paused == speed.paused {
        return;
    }

    if chosen.paused {
        virtual_time.pause();
    } else {
        virtual_time.unpause();
        virtual_time.set_relative_speed(chosen.multiplier());
    }
    *speed = chosen;
}

#[derive(Component)]
struct SpeedReadout;

fn spawn_speed_readout(mut commands: Commands) {
    commands.spawn((
        Name::new("Playback speed"),
        SpeedReadout,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            right: Val::Px(12.0),
            ..default()
        },
    ));
}

/// Solo se anuncia lo que no es normal: a 1× la pantalla no dice nada.
fn draw_speed_readout(
    speed: Res<PlaybackSpeed>,
    mut readout: Query<&mut Text, With<SpeedReadout>>,
) {
    let Ok(mut text) = readout.single_mut() else {
        return;
    };
    let drawn = if speed.is_paused() {
        "|| pausa".to_string()
    } else if speed.step == NORMAL {
        String::new()
    } else {
        format!("x{}", speed.multiplier())
    };
    if text.0 != drawn {
        text.0 = drawn;
    }
}
