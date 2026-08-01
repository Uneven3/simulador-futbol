//! Los ojos: qué ve cada jugador y qué recuerda de lo que dejó de ver.
//!
//! Es el único que escribe `ObservationMemory` y `Beliefs`, y desde que
//! `select_player_movement` lee lo segundo, los veintidós deciden sobre campos
//! distintos. El balón sigue siendo omnisciente: `Ball.predictions` es la
//! trayectoria futura real y todo el mundo la lee.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;

use crate::SimulationSet;
use crate::team_tactics::PlayerReading;
use football_domain::{
    Ball, Facing, Observation, ObservationMemory, Player, PlayerId, Position, Velocity, Vision,
    can_see,
};

pub struct PerceptionPlugin;

impl Plugin for PerceptionPlugin {
    fn build(&self, app: &mut App) {
        // Se mira, se hace uno una idea, y después se decide (§3).
        app.init_resource::<Beliefs>().add_systems(
            FixedUpdate,
            (observe_the_pitch, believe_the_pitch)
                .chain()
                .in_set(SimulationSet::Players)
                .before(crate::player_decisions::select_player_movement),
        );
    }
}

/// El campo tal y como se lo imagina cada jugador: uno por cabeza y no uno para
/// todos, que era la omnisciencia. Dos compañeros pueden decidir sobre campos
/// distintos, y esa diferencia es el error. Los `Vec` se reutilizan (§12).
#[derive(Resource, Debug, Default)]
pub struct Beliefs {
    by_player: Vec<(PlayerId, Vec<PlayerReading>)>,
    ball_error: Vec<(PlayerId, Vec2)>,
}

impl Beliefs {
    pub fn of(&self, who: PlayerId) -> &[PlayerReading] {
        self.by_player
            .iter()
            .find(|(id, _)| *id == who)
            .map_or(&[], |(_, readings)| readings.as_slice())
    }

    /// Lo que este jugador se equivoca sobre dónde está el balón, en metros.
    ///
    /// Hoy solo gobierna hacia dónde mira y qué se dibuja: sumarlo también a la
    /// trayectoria que persigue dejó el partido sin un solo tiro, porque un
    /// error de un metro impide un contacto que se decide en sesenta y cinco.
    pub fn ball_error_of(&self, who: PlayerId) -> Vec2 {
        self.ball_error
            .iter()
            .find(|(id, _)| *id == who)
            .map_or(Vec2::ZERO, |(_, error)| *error)
    }

    fn remember_ball_error(&mut self, who: PlayerId, error: Vec2) {
        match self.ball_error.iter_mut().find(|(id, _)| *id == who) {
            Some((_, known)) => *known = error,
            None => self.ball_error.push((who, error)),
        }
    }

    fn slot_for(&mut self, who: PlayerId) -> &mut Vec<PlayerReading> {
        if let Some(index) = self.by_player.iter().position(|(id, _)| *id == who) {
            let readings = &mut self.by_player[index].1;
            readings.clear();
            return readings;
        }
        self.by_player.push((who, Vec::with_capacity(22)));
        &mut self.by_player.last_mut().expect("acaba de insertarse").1
    }
}

/// Lo que cada jugador cree del resto: donde los vio, adelantados hasta ahora.
/// A quien nunca ha visto no está —no puede recibir un pase suyo ni contar como
/// marca—, y eso es lo que separa esto de la verdad que se leía antes.
pub fn believe_the_pitch(
    time: Res<Time>,
    mut beliefs: ResMut<Beliefs>,
    ball_query: Query<&Position, (With<Ball>, Without<Player>)>,
    who_is_who: Query<(&Player, &ObservationMemory, &Position, &Velocity)>,
) {
    let now = time.elapsed();
    let ball_now = ball_query.single().ok().map(|position| position.on_pitch());
    // los datos que no se observan porque no cambian: quién es cada dorsal
    let identities: Vec<(PlayerId, &Player)> = who_is_who
        .iter()
        .map(|(player, ..)| (player.id, player))
        .collect();

    for (player, memory, position, velocity) in who_is_who.iter() {
        // quien no lo ve lo sitúa donde lo dejó, adelantado a ojo
        let believed_ball = memory.ball.map(|seen| seen.projected_to(now));
        let error = match (believed_ball, ball_now) {
            (Some(believed), Some(actual)) => believed - actual,
            // sin haberlo visto nunca no hay creencia que corregir
            _ => Vec2::ZERO,
        };
        beliefs.remember_ball_error(player.id, error);

        let readings = beliefs.slot_for(player.id);
        // uno se conoce a sí mismo sin tener que mirarse
        readings.push(PlayerReading {
            id: player.id,
            playing_position: player.position,
            role: player.role,
            pos: position.on_pitch(),
            vel: velocity.0.truncate(),
            formation_slot: player.formation_slot,
        });
        for (seen_id, seen) in memory.everyone() {
            let Some((_, known)) = identities.iter().find(|(id, _)| *id == seen_id) else {
                continue;
            };
            readings.push(PlayerReading {
                id: seen_id,
                playing_position: known.position,
                role: known.role,
                pos: seen.projected_to(now),
                vel: seen.velocity,
                formation_slot: known.formation_slot,
            });
        }
    }
}

/// Lo que cada jugador ve este tick entra en su memoria; lo que no, se queda
/// como estaba y envejece solo.
pub fn observe_the_pitch(
    time: Res<Time>,
    ball_query: Query<(&Position, &Ball), Without<Player>>,
    bodies: Query<(&Position, &Player, &Velocity), Without<Ball>>,
    mut watchers: Query<(&Position, &Facing, &Player, &Vision, &mut ObservationMemory)>,
) {
    let now = time.elapsed();
    let ball = ball_query
        .single()
        .ok()
        .map(|(position, ball)| (position.on_pitch(), ball.momentum.truncate()));

    for (position, facing, watcher, vision, mut memory) in watchers.iter_mut() {
        let eyes = position.on_pitch();

        for (other_position, other, velocity) in bodies.iter() {
            if other.id == watcher.id {
                continue;
            }
            let spot = other_position.on_pitch();
            if !can_see(eyes, facing.0, spot, vision) {
                continue;
            }
            memory.remember(
                other.id,
                Observation {
                    spot,
                    velocity: velocity.0.truncate(),
                    seen_at: now,
                },
            );
        }

        if let Some((spot, momentum)) = ball
            && can_see(eyes, facing.0, spot, vision)
        {
            memory.ball = Some(Observation {
                spot,
                velocity: momentum,
                seen_at: now,
            });
        }
    }
}
