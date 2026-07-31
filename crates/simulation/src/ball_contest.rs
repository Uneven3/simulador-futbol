//! Quién tiene el balón: se escapa, se disputa, se roba o se recoge.
//!
//! Eran cuatro responsabilidades dentro de un sistema de 461 líneas, y por eso
//! ningún parámetro de la disputa se podía girar sin mover los otros tres
//! (`ARCHITECTURE.md` §6). Ahora cada una es un sistema encadenado, y todos
//! leen el mismo hecho intermedio: quién está en contacto con el balón.
//!
//! El modelo de interacción es el del original: el balón nunca está pegado a un
//! jugador, cada contacto es un toque discreto que le fija un momentum. La
//! posesión es posicional — se tiene mientras se está cerca.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use std::time::Duration;

use crate::SimulationSet;
use crate::diagnostics::{MatchFact, MatchTelemetry, PossessionCause};
use crate::player_movement::dribble_direction;
use football_domain::tuning::ContestTuning;
use football_domain::{
    Attributes, Ball, BallTouched, MatchState, MatchTuning, PitchConfig, Player, PlayerId,
    PlayerMatchState, PlayerRegistry, Position, PossessionDesignation, PotentialFoul, SetPiece,
    TeamId, Velocity,
};

/// Lo que los sistemas del balón necesitan de un cuerpo, más lo único que le
/// escriben: cuándo tocó el balón por última vez.
pub type BallSystemBody = (
    Entity,
    &'static Position,
    &'static Player,
    &'static Attributes,
    &'static mut PlayerMatchState,
    &'static Velocity,
);

/// El balón, tal y como lo tocan los sistemas de esta capa.
pub type BallBody = (&'static mut Position, &'static mut Ball);

/// Tocar el balón necesita siempre lo mismo: los cuerpos, cómo llegar a uno
/// desde una identidad de dominio, y los dos sitios donde queda constancia del
/// toque. Van juntos para que un sistema declare una dependencia en vez de
/// cuatro (§8).
#[derive(SystemParam)]
pub struct Touching<'w, 's> {
    pub players: Query<'w, 's, BallSystemBody, Without<Ball>>,
    pub registry: Res<'w, PlayerRegistry>,
    pub telemetry: ResMut<'w, MatchTelemetry>,
    pub touched: MessageWriter<'w, BallTouched>,
    pub fouls: MessageWriter<'w, PotentialFoul>,
}

/// La configuración bajo la que se juega este partido.
#[derive(SystemParam)]
pub struct MatchSettings<'w> {
    pub pitch: Res<'w, PitchConfig>,
    pub tuning: Res<'w, MatchTuning>,
}

/// Quién está en contacto con el balón este tick, y a qué distancia efectiva.
///
/// Es un hecho derivado, no estado del partido: lo escribe
/// [`select_ball_challenger`] y lo consumen la entrada y la recogida. Guarda
/// una identidad de dominio y no una `Entity`, que solo vale para este tick
/// (ley 4).
#[derive(Resource, Debug, Default)]
pub struct BallContest {
    pub challenger: Option<PlayerId>,
    /// Distancia al balón con la que ganó la disputa: la real, salvo para el
    /// receptor de un pase, que estira la pierna.
    pub distance: f32,
}

/// Quien lo tocó, y qué cuerpo es ahora mismo. La pareja viaja junta porque una
/// es la identidad y el otro solo la forma de alcanzarla este tick.
#[derive(Clone, Copy)]
struct Toucher {
    player: PlayerId,
    body: Entity,
}

/// El partido está parado: nadie disputa nada.
fn play_is_stopped(match_state: Res<MatchState>) -> bool {
    match_state.set_piece != SetPiece::None
}

/// La posesión es posicional: si el balón se le escapó al poseedor, vuelve a
/// estar suelto.
pub fn release_escaped_ball(
    mut match_state: ResMut<MatchState>,
    settings: MatchSettings,
    ball_query: Query<&Position, (With<Ball>, Without<Player>)>,
    mut touching: Touching,
) {
    let Ok(ball_position) = ball_query.single() else {
        return;
    };
    let Some(possessor) = match_state.possession_player else {
        return;
    };
    let ball_pos_2d = ball_position.on_pitch();

    let escaped = match touching
        .registry
        .body(possessor)
        .map(|body| touching.players.get(body))
    {
        Some(Ok((_, position, ..))) => {
            position.on_pitch().distance(ball_pos_2d)
                > settings.tuning.contest.possession_escape_distance
        }
        _ => true,
    };
    if escaped {
        touching.telemetry.record(MatchFact::PossessionLost {
            player: possessor,
            at: ball_pos_2d,
        });
        match_state.possession_player = None;
    }
}

/// El jugador en contacto con el balón, si hay alguno.
///
/// El receptor de un pase alcanza más lejos y gana los empates dentro de su
/// radio: es el sustituto de las animaciones de control del original, que
/// estiran una pierna. Sin eso la recepción es cara o cruz contra el marcador
/// que está uno o dos metros por detrás, y casi ningún pase se completa.
pub fn select_ball_challenger(
    match_state: Res<MatchState>,
    offside_records: Res<football_domain::OffsideRecords>,
    settings: MatchSettings,
    mut contest: ResMut<BallContest>,
    ball_query: Query<&Position, (With<Ball>, Without<Player>)>,
    players: Query<(&Position, &Player), Without<Ball>>,
) {
    *contest = BallContest::default();
    let Ok(ball_position) = ball_query.single() else {
        return;
    };
    let tuning = &settings.tuning.contest;
    let ball_pos = ball_position.0;
    let ball_pos_2d = ball_position.on_pitch();

    let mut closest_distance = f32::MAX;
    for (player_position, player) in players.iter() {
        // a quien el árbitro anotó en fuera de juego en el último toque no le
        // vale disputar: el silbato sonaría en cuanto la tocase
        if offside_records.team == Some(player.id.team)
            && offside_records
                .players
                .iter()
                .any(|(id, _)| *id == player.id)
        {
            continue;
        }

        let distance = ball_pos_2d.distance(player_position.on_pitch());
        let is_receiver = match_state.pass_target == Some(player.id);
        let reach = if is_receiver {
            tuning.receiver_trap_reach
        } else {
            tuning.loose_ball_reach
        };
        if distance >= reach || ball_pos.z >= tuning.max_touch_height {
            continue;
        }

        let effective = if is_receiver {
            distance - tuning.receiver_tie_break
        } else {
            distance
        };
        if effective < closest_distance {
            closest_distance = effective;
            contest.challenger = Some(player.id);
        }
    }
    contest.distance = closest_distance.max(0.0);
}

/// En qué acaba una entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tackle {
    /// Llegó antes al balón: se lo lleva.
    WonTheBall,
    /// Llegó al hombre: falta.
    Foul,
    /// Ni una cosa ni la otra.
    Missed,
}

/// Qué sale de ir a por un balón que lleva otro.
///
/// El orden importa y es el del fútbol: primero se mira si llegó al balón,
/// porque quien lo gana no comete falta por mucho que hubiera contacto. Solo
/// cuando no lo gana empieza a contar que estuviera encima del hombre.
pub fn judge_tackle(
    challenger_to_ball: f32,
    carrier_to_ball: f32,
    bodies_apart: f32,
    tuning: &ContestTuning,
) -> Tackle {
    let reached_the_ball = challenger_to_ball < tuning.tackle_contact_distance;
    if reached_the_ball && challenger_to_ball < carrier_to_ball * tuning.duel_advantage {
        return Tackle::WonTheBall;
    }
    if reached_the_ball && bodies_apart < tuning.foul_contact_distance {
        return Tackle::Foul;
    }
    Tackle::Missed
}

/// Robarle el balón a quien lo lleva.
///
/// Tres condiciones, y las tres existen porque aquí no hay cuerpo que proteja
/// el balón como en el original: solo el jugador designado entra, tiene que
/// estar genuinamente más cerca que el portador, y el balón tiene que estar
/// robable — fuera del pie o retenido tanto tiempo que la presión justifique la
/// pérdida. Sin ellas los dos designados se intercambian el balón en cada
/// ventana de enfriamiento y el partido degenera en un metrónomo de robos.
pub fn resolve_tackle(
    mut match_state: ResMut<MatchState>,
    designation: Res<PossessionDesignation>,
    settings: MatchSettings,
    contest: Res<BallContest>,
    time: Res<Time>,
    mut ball_query: Query<BallBody, Without<Player>>,
    mut touching: Touching,
) {
    let (Some(challenger), Some(current)) = (contest.challenger, match_state.possession_player)
    else {
        return;
    };
    let sides = match_state.sides;
    if challenger == current || challenger.team == current.team {
        return;
    }
    let Ok((mut ball_position, mut ball)) = ball_query.single_mut() else {
        return;
    };
    let Some(Ok((_, carrier_position, ..))) = touching
        .registry
        .body(current)
        .map(|body| touching.players.get(body))
    else {
        return;
    };

    let tuning = &settings.tuning.contest;
    let now = time.elapsed();
    let ball_pos_2d = ball_position.on_pitch();
    let carrier_distance = carrier_position.on_pitch().distance(ball_pos_2d);
    let held_for = now.saturating_sub(match_state.possession_since);

    let is_designated_tackler = designation.designated[challenger.team] == Some(challenger);
    if !is_designated_tackler {
        return;
    }

    let bodies_apart = carrier_position.on_pitch().distance(
        touching
            .registry
            .body(challenger)
            .and_then(|body| touching.players.get(body).ok())
            .map_or(Vec2::splat(f32::MAX), |(_, position, ..)| {
                position.on_pitch()
            }),
    );
    match judge_tackle(contest.distance, carrier_distance, bodies_apart, tuning) {
        Tackle::Missed => return,
        Tackle::Foul => {
            touching.fouls.write(PotentialFoul {
                by: challenger,
                on: current,
                at: ball_position.0,
            });
            return;
        }
        Tackle::WonTheBall => {}
    }

    let ball_is_stealable = carrier_distance > tuning.shielding_release_distance
        || held_for > tuning.shielding_release_time;
    if !ball_is_stealable {
        return;
    }

    // quien acaba de perderlo espera más para recuperarlo, o los dos designados
    // se lo intercambian en cada ventana
    let cooldown = if match_state.previous_possessor == Some(challenger) {
        tuning.regain_cooldown
    } else {
        tuning.steal_cooldown
    };
    if held_for <= cooldown {
        return;
    }

    let Some(challenger_body) = touching.registry.body(challenger) else {
        return;
    };
    match_state.previous_possessor = Some(current);
    take_possession(&mut match_state, challenger, now);
    touching.telemetry.record(MatchFact::PossessionGained {
        player: challenger,
        from: Some(current),
        cause: PossessionCause::Tackle,
        at: ball_pos_2d,
    });
    control_touch(
        &mut ball,
        &mut ball_position,
        Toucher {
            player: challenger,
            body: challenger_body,
        },
        now,
        sides,
        tuning,
        &mut touching,
    );
}

/// Recoger un balón que no es de nadie.
///
/// Tres frenos, todos por la misma razón que la entrada: sin animaciones, el
/// balón cambiaría de dueño cada pocos milisegundos. Hay un enfriamiento
/// después de cualquier golpeo, uno más largo para quien acaba de golpearlo, y
/// la prioridad del último que la jugó (`GetLastTouchBias` del original), que
/// obliga al rival a llegar claramente antes.
pub fn collect_loose_ball(
    mut match_state: ResMut<MatchState>,
    settings: MatchSettings,
    contest: Res<BallContest>,
    time: Res<Time>,
    mut ball_query: Query<BallBody, Without<Player>>,
    mut touching: Touching,
) {
    if match_state.possession_player.is_some() {
        return;
    }
    let sides = match_state.sides;
    let Some(challenger) = contest.challenger else {
        return;
    };
    let Ok((mut ball_position, mut ball)) = ball_query.single_mut() else {
        return;
    };

    let tuning = &settings.tuning.contest;
    let now = time.elapsed();
    let since_touch = now.saturating_sub(ball.last_touch_at);
    let is_last_toucher = ball.last_touch_player == Some(challenger);

    if since_touch <= tuning.loose_ball_cooldown {
        return;
    }
    if is_last_toucher && since_touch <= tuning.own_ball_cooldown {
        return;
    }
    let ball_pos_2d = ball_position.on_pitch();
    if !is_last_toucher
        && since_touch < tuning.touch_bias_window
        && last_toucher_keeps_priority(
            &ball,
            ball_pos_2d,
            challenger,
            contest.distance,
            tuning,
            &touching,
        )
    {
        return;
    }

    let Some(challenger_body) = touching.registry.body(challenger) else {
        return;
    };
    touching.telemetry.record(MatchFact::PossessionGained {
        player: challenger,
        from: None,
        cause: PossessionCause::LooseBall,
        at: ball_pos_2d,
    });
    match_state.previous_possessor = None;
    take_possession(&mut match_state, challenger, now);
    control_touch(
        &mut ball,
        &mut ball_position,
        Toucher {
            player: challenger,
            body: challenger_body,
        },
        now,
        sides,
        tuning,
        &mut touching,
    );
}

/// Si el último que la jugó sigue encima del balón, un rival tiene que llegar
/// claramente antes para quitársela; si no, el regateador pierde cada toque a
/// cara o cruz.
fn last_toucher_keeps_priority(
    ball: &Ball,
    ball_pos_2d: Vec2,
    challenger: PlayerId,
    challenger_distance: f32,
    tuning: &ContestTuning,
    touching: &Touching,
) -> bool {
    let Some(last_toucher) = ball.last_touch_player else {
        return false;
    };
    if last_toucher.team == challenger.team {
        return false;
    }
    let Some(Ok((_, toucher_position, ..))) = touching
        .registry
        .body(last_toucher)
        .map(|body| touching.players.get(body))
    else {
        return false;
    };
    let toucher_distance = toucher_position.on_pitch().distance(ball_pos_2d);
    toucher_distance < tuning.touch_bias_distance
        && challenger_distance > toucher_distance - tuning.touch_bias_margin
}

/// El balón pasa a ser de este jugador, ahora.
fn take_possession(match_state: &mut MatchState, player: PlayerId, now: Duration) {
    match_state.possession_player = Some(player);
    match_state.possession_team = Some(player.team);
    match_state.possession_since = now;
    match_state.pass_target = None;
}

/// El primer toque al ganar la posesión: mata casi todo el momentum para que el
/// balón se quede en el pie (sustituto de las animaciones de control).
///
/// Y es un toque DIRIGIDO: el balón se coloca hacia donde el portador quiere
/// ir, no hacia donde venía corriendo. Un control en la dirección cruda de la
/// carrera, junto a la banda, echa el balón fuera y encadena saques de banda
/// sin fin.
fn control_touch(
    ball: &mut Ball,
    ball_position: &mut Position,
    toucher: Toucher,
    now: Duration,
    sides: football_domain::PitchSides,
    tuning: &ContestTuning,
    touching: &mut Touching,
) {
    let Toucher { player, body } = toucher;
    let trap_momentum = if let Ok((_, position, _, _, _, velocity)) = touching.players.get(body) {
        let my_pos = position.on_pitch();
        let my_vel = Vec2::new(velocity.0.x, velocity.0.y);
        let all_players: Vec<(TeamId, Vec2, Vec2)> = touching
            .players
            .iter()
            .map(|(_, p_position, p, _, _, v)| {
                (p.id.team, p_position.on_pitch(), Vec2::new(v.0.x, v.0.y))
            })
            .collect();
        let dir = dribble_direction(my_pos, my_vel, player.team, sides, &all_players);
        // se coloca a paso de conducción (AI_GetBallControlMovement): un control
        // muerto aparca el balón en medio del duelo e invita al robo
        let (min_trap, max_trap) = tuning.trap_speed_range;
        let speed = (my_vel.length() * tuning.trap_speed_from_run).clamp(min_trap, max_trap);
        Vec3::new(dir.x, dir.y, 0.0) * speed
    } else {
        ball.momentum * 0.2
    };

    crate::ball_physics::touch_ball(ball, ball_position, trap_momentum);
    ball.set_rotation(0.0, 0.0, 0.0, 1.0);
    ball.last_touch_team = Some(player.team);
    ball.last_touch_player = Some(player);
    ball.last_touch_at = now;
    if let Ok((.., mut player_state, _)) = touching.players.get_mut(body) {
        player_state.last_touch_at = now;
    }
    touching.touched.write(BallTouched { player });
    touching.telemetry.record(MatchFact::Touched {
        player,
        deliberate: true,
    });
}

/// Las dos mitades de un toque, en orden. Primero se decide de quién es el
/// balón; solo después puede su dueño hacer algo con él.
///
/// Son sets y no un `.chain()` suelto para que lo que venga después —faltas,
/// que son un incidente durante la disputa— entre como sistema propio en el set
/// que le toca, sin editar ninguno de estos (§7).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BallTouchSet {
    /// De quién es el balón: escapadas, contacto, entrada y recogida.
    Contest,
    /// Qué hace con él quien lo tiene.
    Release,
}

/// La disputa del balón. Sale de aquí sabiendo quién lo tiene.
pub struct BallContestPlugin;

impl Plugin for BallContestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BallContest>()
            .add_message::<PotentialFoul>()
            .configure_sets(
                FixedUpdate,
                (BallTouchSet::Contest, BallTouchSet::Release)
                    .chain()
                    .in_set(SimulationSet::Kicks),
            )
            .add_systems(
                FixedUpdate,
                (
                    crate::goalkeeping::goalkeepers_save_shots,
                    release_escaped_ball,
                    select_ball_challenger,
                    resolve_tackle,
                    collect_loose_ball,
                )
                    .chain()
                    .run_if(not(play_is_stopped))
                    .in_set(BallTouchSet::Contest),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quien llega al balón no comete falta, por mucho que se lleve al hombre
    /// por delante. Es el orden del fútbol y el de la función.
    #[test]
    fn winning_the_ball_is_never_a_foul() {
        let tuning = ContestTuning::default();
        let touching_bodies = tuning.foul_contact_distance * 0.5;

        assert_eq!(
            judge_tackle(0.1, 1.0, touching_bodies, &tuning),
            Tackle::WonTheBall
        );
    }

    /// Ir a por el balón, no llegar y llevarse al hombre sí lo es.
    #[test]
    fn missing_the_ball_and_hitting_the_man_is_a_foul() {
        let tuning = ContestTuning::default();
        let inside = tuning.tackle_contact_distance * 0.9;
        let touching_bodies = tuning.foul_contact_distance * 0.5;

        assert_eq!(
            judge_tackle(inside, inside, touching_bodies, &tuning),
            Tackle::Foul,
            "entró tarde y encima, y no se pitó"
        );
    }

    /// Y no llegar a nada no es nada: sin contacto no hay falta.
    #[test]
    fn a_tackle_that_touches_nothing_is_not_a_foul() {
        let tuning = ContestTuning::default();
        let inside = tuning.tackle_contact_distance * 0.9;
        let apart = tuning.foul_contact_distance * 2.0;

        assert_eq!(judge_tackle(inside, inside, apart, &tuning), Tackle::Missed);
        assert_eq!(
            judge_tackle(5.0, 0.2, 0.1, &tuning),
            Tackle::Missed,
            "pitó falta a quien ni fue a por el balón"
        );
    }
}
