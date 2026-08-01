//! Qué hace con el balón quien lo tiene, y cómo sale del pie.
//!
//! La decisión es de `player_decisions`; aquí solo se ejecuta. La separación
//! importa para calibrar: "dispara demasiado" y "dispara mal" son dos defectos
//! distintos, y viven en dos sitios distintos.
//!
//! Las recetas de golpeo son funciones puras que devuelven un [`Kick`]
//! (§8): reciben la situación y el tuning, no el mundo, así que la calibración
//! de MVP 1.75 puede probarlas sin levantar un partido.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use std::time::Duration;

use crate::ball_contest::{BallBody, BallTouchSet, MatchSettings, Touching};
use crate::diagnostics::{MatchFact, ReleaseKind};
use crate::player_decisions::{self, OnBallAction, PassKind};
use crate::player_movement::dribble_direction;
use crate::team_tactics::PlayerReading;
use football_domain::math::{normalized_clamp, normalized_or_2d};
use football_domain::tuning::{
    ClearanceTuning, ContestTuning, PassingTuning, ShootingTuning, StrikingTuning,
};
use football_domain::{
    Ball, BallTouched, MatchRng, MatchState, PitchConfig, Player, PlayerId, PlayingPosition,
    Position, PossessionDesignation, SetPiece, TeamId,
};

/// Un golpeo resuelto: con qué momentum sale el balón, con qué efecto, y hacia
/// dónde se dijo que iba (que es lo que el diagnóstico necesita para juzgar
/// después si llegó).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Kick {
    pub momentum: Vec3,
    pub spin: Vec3,
    pub aim: Vec2,
}

/// El disparo, tal y como lo ejecuta el port. Apunta a la LÍNEA de gol y no a
/// un punto delante de ella, o todo disparo en diagonal se marcha fuera. La
/// dispersión es lo único que lo separa de un golpeo perfecto.
pub fn solve_shot(
    from: Vec2,
    target_y: f32,
    attacking_towards_x: f32,
    shot_technique: f32,
    tuning: &ShootingTuning,
    rng: &mut MatchRng,
) -> Kick {
    let goal_x = 55.0 * attacking_towards_x;
    let spread = 1.0 - shot_technique;
    let aim_y = target_y * tuning.aim_centre_pull + rng.range(-spread, spread);
    let direction = (Vec2::new(goal_x, aim_y) - from).normalize_or_zero();

    let distance_factor = normalized_clamp(
        from.distance(Vec2::new(goal_x, 0.0)),
        0.0,
        tuning.power_distance_range,
    );
    let lift = tuning.lift + distance_factor * tuning.lift_distance_gain;
    let kick_direction = Vec3::new(direction.x, direction.y, lift).normalize_or_zero();

    // original: desiredPower = random(0.7, 1.0) * (0.6 + goalDist * 0.4)
    let (min_draw, max_draw) = tuning.power_random_range;
    let power = rng.range(min_draw, max_draw)
        * (tuning.power_distance_base + distance_factor * tuning.power_distance_gain)
        * tuning.power_scale
        + tuning.power_floor;

    let side_spin = -direction.y.signum() * kick_direction.x.signum() * tuning.sidespin;
    Kick {
        momentum: kick_direction * power,
        spin: Vec3::new(
            tuning.topspin * kick_direction.y,
            tuning.topspin * kick_direction.x,
            side_spin,
        ),
        aim: Vec2::new(goal_x, aim_y),
    }
}

/// El pase, resuelto contra la física real para que LLEGUE: un balón que muere
/// en el receptor recorre sus últimos metros arrastrándose, y cualquier rival
/// recoge esa cola lenta.
pub fn solve_pass(
    from: Vec2,
    ball_pos: Vec3,
    aim: Vec2,
    kind: PassKind,
    pitch: &PitchConfig,
    tuning: &PassingTuning,
) -> Kick {
    // El alcance se mide desde el jugador y la trayectoria desde el balón: son
    // dos puntos a medio metro uno del otro, y confundirlos cambia el vuelo de
    // los pases altos lo bastante para mover un partido entero.
    let distance = from.distance(aim);
    let (lift, pace) = match kind {
        PassKind::Short => (tuning.short_lift, tuning.short_pace),
        PassKind::Long => (tuning.long_lift, tuning.long_pace),
        PassKind::High => (
            tuning.high_lift
                - normalized_clamp(distance, 0.0, 60.0) * tuning.high_lift_range_relief,
            tuning.high_pace,
        ),
    };
    Kick {
        momentum: crate::ball_physics::solve_pass_momentum(pitch, ball_pos, aim, lift, pace),
        spin: Vec3::ZERO,
        aim,
    }
}

/// El despeje (`_AddPanicPass` del original): lejos, hacia adelante y fuera del
/// centro.
pub fn solve_clearance(
    from: Vec2,
    running: Vec2,
    attacking_towards_x: f32,
    tuning: &ClearanceTuning,
) -> Kick {
    let forward = attacking_towards_x;
    let y_side = if running.y >= 0.0 { 1.0 } else { -1.0 };
    let direction = normalized_or_2d(running, Vec2::new(forward, 0.0));
    let away = (normalized_or_2d(direction * Vec2::new(0.8, 1.0), Vec2::new(forward, 0.0))
        + Vec2::new(forward * 0.7, y_side * 0.5))
    .normalize_or_zero();
    let kick_direction = Vec3::new(away.x, away.y, tuning.lift).normalize_or_zero();
    Kick {
        momentum: kick_direction * tuning.power,
        spin: Vec3::ZERO,
        aim: from + away * 30.0,
    }
}

/// El toque de conducción: el balón rueda libre y el portador lo persigue. En
/// tráfico se acorta a paso de conducción, porque un toque a velocidad punta
/// entre rivales rueda tres metros y le regala la posesión al contrario.
pub fn solve_knock_on(
    from: Vec2,
    running: Vec2,
    direction: Vec2,
    nearest_opponent: f32,
    tuning: &ContestTuning,
) -> Kick {
    let reachable = if nearest_opponent < tuning.knock_on_traffic_distance {
        tuning.carry_pace_in_traffic
    } else {
        tuning.carry_pace
    };
    // el balón sale por debajo de la carrera para que el siguiente paso lo
    // alcance: por encima de ella se va solo y deja de ser conducción
    let speed = (running.length() * tuning.knock_on_from_run).clamp(1.5, reachable);
    Kick {
        momentum: Vec3::new(direction.x, direction.y, 0.0) * speed,
        spin: Vec3::ZERO,
        aim: from + direction * 1.5,
    }
}

/// Lo que un jugador ya empezó a hacer con el balón y todavía no ha hecho: la
/// acción y el instante en que el pie llega. Mientras dura, el partido sigue, y
/// por eso comprometerse cuesta algo.
#[derive(Component, Debug, Clone, Copy)]
pub struct ActionCommitment {
    action: OnBallAction,
    contact_at: Duration,
}

/// Con qué se resuelve un golpeo: el estado del partido que va a cambiar, los
/// parámetros, el azar y el reloj. Las dos mitades del golpeo —comprometerse y
/// darlo— necesitan exactamente esto (§8).
#[derive(SystemParam)]
pub struct Striking<'w> {
    pub match_state: ResMut<'w, MatchState>,
    pub settings: MatchSettings<'w>,
    pub rng: ResMut<'w, MatchRng>,
    pub time: Res<'w, Time>,
}

/// Cuánto tarda el pie en llegar al balón. Conducir no es golpear: el toque de
/// conducción es la propia carrera y sale en el acto.
fn windup_of(action: OnBallAction, tuning: &StrikingTuning) -> Duration {
    match action {
        OnBallAction::Shot { .. } => tuning.shot_windup,
        OnBallAction::Pass { .. } => tuning.pass_windup,
        OnBallAction::PanicClear => tuning.clearance_windup,
        OnBallAction::Dribble => Duration::ZERO,
    }
}

/// El poseedor decide qué hacer con el balón, y se compromete a ello.
///
/// Decidir y golpear eran el mismo instante, así que un pase no podía salir mal
/// por nada que pasara entre una cosa y la otra: no había entre.
pub fn commit_to_an_action(
    mut commands: Commands,
    striking: Striking,
    designation: Res<PossessionDesignation>,
    committed: Query<&ActionCommitment>,
    ball_query: Query<(&Position, &Ball), Without<Player>>,
    touching: Touching,
) {
    let Striking {
        match_state,
        settings,
        mut rng,
        time,
    } = striking;
    if match_state.set_piece != SetPiece::None {
        return;
    }
    let Some(possessor) = match_state.possession_player else {
        return;
    };
    let Ok((ball_position, ball)) = ball_query.single() else {
        return;
    };
    let Some(possessor_body) = touching.registry.body(possessor) else {
        return;
    };
    if committed.get(possessor_body).is_ok() {
        return;
    }
    let Ok((_, player_position, player, ..)) = touching.players.get(possessor_body) else {
        return;
    };

    let contest = &settings.tuning.contest;
    let now = time.elapsed();
    let ball_pos = ball_position.0;
    let from = player_position.on_pitch();
    let is_goalkeeper = player.position == PlayingPosition::Goalkeeper;

    // el toque exige el balón en el pie
    let in_reach = ball_position.on_pitch().distance(from) < contest.ball_at_feet_distance
        && ball_pos.z < contest.ball_at_feet_height;
    // Los porteros golpean sin demora, y una suelta deliberada solo necesita
    // reacción corta: obligar a esperar a un portador presionado alimenta el
    // bucle de robos. La conducción sí mantiene la cadencia lenta.
    let since_touch = now.saturating_sub(ball.last_touch_at);
    let can_decide = since_touch > contest.decision_cadence || is_goalkeeper;
    if !(in_reach && can_decide) {
        return;
    }

    let readings = readings_of(&touching);
    let sides = match_state.sides;
    let offside_line =
        player_decisions::offside_line(&readings, possessor.team, sides, ball_pos.x, 0.0);
    let action = player_decisions::decide_on_ball_action(
        &readings,
        possessor,
        ball,
        &designation,
        now.saturating_sub(match_state.possession_since),
        offside_line,
        sides,
        &settings.tuning,
        &mut rng,
    );

    // Quien pone el balón en juego no lo conduce: lo pasa o lo patea, que es lo
    // que dicen las leyes de cada reanudación. Sin esto el que saca se quedaba
    // el balón y se iba con él.
    let must_release = match_state.restart_taker == Some(possessor);
    let action = if must_release && matches!(action, OnBallAction::Dribble) {
        OnBallAction::PanicClear
    } else {
        action
    };

    // el knock-on de la conducción mantiene su propia cadencia, más lenta
    let can_knock_on = since_touch > contest.knock_on_cadence || is_goalkeeper;
    if matches!(action, OnBallAction::Dribble) && !can_knock_on {
        return;
    }

    commands.entity(possessor_body).insert(ActionCommitment {
        action,
        contact_at: now + windup_of(action, &settings.tuning.striking),
    });
}

/// Cómo está el mundo ahora mismo, para quien tenga que resolver algo con él.
fn readings_of(touching: &Touching) -> Vec<PlayerReading> {
    touching
        .players
        .iter()
        .map(|(_, position, p, _, _, v)| PlayerReading {
            id: p.id,
            playing_position: p.position,
            role: p.role,
            pos: position.on_pitch(),
            vel: Vec2::new(v.0.x, v.0.y),
            formation_slot: p.formation_slot,
        })
        .collect()
}

/// El pie llega al balón, y lo que se decidió hace dos décimas ocurre ahora. O
/// no: si por el camino se lo quitaron, el golpeo se queda sin balón, como le
/// pasa a quien recibe una entrada mientras arma la pierna.
pub fn complete_committed_strike(
    mut commands: Commands,
    striking: Striking,
    committed: Query<(Entity, &ActionCommitment)>,
    mut ball_query: Query<BallBody, Without<Player>>,
    mut touching: Touching,
) {
    let Striking {
        mut match_state,
        settings,
        mut rng,
        time,
    } = striking;
    let now = time.elapsed();
    let Ok((mut ball_position, mut ball)) = ball_query.single_mut() else {
        return;
    };

    for (body, commitment) in committed.iter() {
        let Ok((_, player_position, player, stats, _, velocity)) = touching.players.get(body)
        else {
            commands.entity(body).remove::<ActionCommitment>();
            continue;
        };
        let possessor = player.id;
        let from = player_position.on_pitch();
        let running = Vec2::new(velocity.0.x, velocity.0.y);
        let shot_technique = stats.shot_technique;

        let contest = &settings.tuning.contest;
        let ball_pos = ball_position.0;
        let still_his = match_state.possession_player == Some(possessor)
            && match_state.set_piece == SetPiece::None;
        let in_reach = ball_position.on_pitch().distance(from) < contest.ball_at_feet_distance
            && ball_pos.z < contest.ball_at_feet_height;
        if !(still_his && in_reach) {
            commands.entity(body).remove::<ActionCommitment>();
            continue;
        }
        if now < commitment.contact_at {
            continue;
        }
        commands.entity(body).remove::<ActionCommitment>();

        let readings = readings_of(&touching);
        let sides = match_state.sides;
        let attacking_towards_x = sides.attacking_x(possessor.team);

        let (kick, release, keeps_possession) = match commitment.action {
            OnBallAction::Shot { target_y } => {
                match_state.pass_target = None;
                (
                    solve_shot(
                        from,
                        target_y,
                        attacking_towards_x,
                        shot_technique,
                        &settings.tuning.shooting,
                        &mut rng,
                    ),
                    ReleaseKind::Shot,
                    false,
                )
            }
            OnBallAction::Pass { target, aim, kind } => {
                match_state.pass_target = Some(target);
                match_state.pass_aim = aim;
                (
                    solve_pass(
                        from,
                        ball_pos,
                        aim,
                        kind,
                        &settings.pitch,
                        &settings.tuning.passing,
                    ),
                    ReleaseKind::Pass,
                    false,
                )
            }
            OnBallAction::PanicClear => {
                match_state.pass_target = None;
                (
                    solve_clearance(
                        from,
                        running,
                        attacking_towards_x,
                        &settings.tuning.clearance,
                    ),
                    ReleaseKind::Clearance,
                    false,
                )
            }
            OnBallAction::Dribble => {
                let bodies: Vec<(TeamId, Vec2, Vec2)> =
                    readings.iter().map(|s| (s.team(), s.pos, s.vel)).collect();
                let direction = dribble_direction(from, running, possessor.team, sides, &bodies);
                let nearest_opponent = readings
                    .iter()
                    .filter(|s| s.team() != possessor.team)
                    .map(|s| s.pos.distance(from))
                    .fold(f32::MAX, f32::min);
                (
                    solve_knock_on(from, running, direction, nearest_opponent, contest),
                    ReleaseKind::DribbleKnock,
                    true,
                )
            }
        };

        strike(
            &mut ball,
            &mut ball_position,
            kick,
            possessor,
            now,
            &mut touching,
        );
        touching.telemetry.record(MatchFact::BallReleased {
            player: possessor,
            kind: release,
            aim: kick.aim,
        });
        if let Ok((.., mut player_state, _)) = touching.players.get_mut(body) {
            player_state.last_touch_at = now;
        }
        if !keeps_possession {
            match_state.possession_player = None;
        }
        // el balón está en juego: ya no hay nadie obligado a ponerlo
        if !matches!(commitment.action, OnBallAction::Dribble) {
            match_state.restart_taker = None;
        }
    }
}

/// El golpeo, aplicado al balón. Todo toque deliberado pasa por aquí.
fn strike(
    ball: &mut Ball,
    ball_position: &mut Position,
    kick: Kick,
    by: PlayerId,
    now: Duration,
    touching: &mut Touching,
) {
    crate::ball_physics::touch_ball(ball, ball_position, kick.momentum);
    ball.set_rotation(kick.spin.x, kick.spin.y, kick.spin.z, 1.0);
    ball.last_touch_team = Some(by.team);
    ball.last_touch_player = Some(by);
    ball.last_touch_at = now;
    touching.touched.write(BallTouched { player: by });
}

/// Lo que el poseedor hace con el balón.
pub struct BallReleasePlugin;

impl Plugin for BallReleasePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (commit_to_an_action, complete_committed_strike)
                .chain()
                .in_set(BallTouchSet::Release),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::tuning::MatchTuning;

    /// El golpeo dejó de ser un instante: hay ticks en los que un jugador ya
    /// decidió y todavía no ha tocado el balón. Sin ese hueco no hay nada que
    /// un rival pueda aprovechar.
    #[test]
    fn there_are_ticks_where_a_strike_is_under_way() {
        use bevy_app::TaskPoolPlugin;
        use bevy_time::{TimePlugin, TimeUpdateStrategy};
        use football_domain::Scenario;
        use football_domain::scenario::TICK;

        let mut app = bevy_app::App::new();
        app.add_plugins((TaskPoolPlugin::default(), TimePlugin));
        app.add_plugins(crate::MatchKernelPlugin::new(Scenario::kick_off()));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));

        let mut ticks_mid_strike = 0;
        for _ in 0..3000 {
            app.update();
            let mut committed = app.world_mut().query::<&ActionCommitment>();
            if committed.iter(app.world()).next().is_some() {
                ticks_mid_strike += 1;
            }
        }

        assert!(
            ticks_mid_strike > 0,
            "en treinta segundos de partido nadie llegó a armar un golpeo"
        );
    }

    /// Conducir es correr con el balón, así que el toque sale en el acto;
    /// golpearlo tiene un tiempo, y ese tiempo es lo que un rival aprovecha.
    #[test]
    fn only_striking_the_ball_takes_time() {
        let tuning = StrikingTuning::default();

        assert_eq!(windup_of(OnBallAction::Dribble, &tuning), Duration::ZERO);
        assert!(windup_of(OnBallAction::PanicClear, &tuning) > Duration::ZERO);
        assert!(
            windup_of(OnBallAction::Shot { target_y: 0.0 }, &tuning)
                > windup_of(
                    OnBallAction::Pass {
                        target: PlayerId::new(TeamId::Home, 9),
                        aim: Vec2::ZERO,
                        kind: PassKind::Short,
                    },
                    &tuning
                ),
            "armar un disparo tenía que costar más que dar un pase corto"
        );
    }

    /// Un disparo desde el borde del área sale hacia la portería contraria, con
    /// potencia dentro de lo que un futbolista pega. Es la receta que MVP 1.75
    /// va a girar, así que conviene que falle aquí y no sesenta mil ticks
    /// después.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "la receta devuelve estas cifras exactas, y el test las repite"
    )]
    fn a_shot_leaves_the_boot_towards_the_goal_being_attacked() {
        let tuning = MatchTuning::default();
        let mut rng = MatchRng::seeded(7);
        let kick = solve_shot(
            Vec2::new(38.0, 4.0),
            0.0,
            1.0,
            0.5,
            &tuning.shooting,
            &mut rng,
        );

        assert!(kick.momentum.x > 0.0, "el disparo va hacia +x");
        assert!(kick.momentum.z > 0.0, "sale despegado del suelo");
        let speed = kick.momentum.length();
        assert!(
            (10.0..40.0).contains(&speed),
            "velocidad de disparo irreal: {speed} m/s"
        );
        assert_eq!(kick.aim.x, 55.0, "se apunta a la línea de gol");
    }

    /// El despeje mezcla hacia dónde corre el defensa con un sesgo hacia
    /// adelante, y abre hacia la banda. Corriendo hacia adelante, sale hacia
    /// adelante y despegado del suelo.
    #[test]
    fn a_clearance_goes_forward_and_out_wide() {
        let tuning = MatchTuning::default();
        let kick = solve_clearance(
            Vec2::new(-45.0, 2.0),
            Vec2::new(2.0, 0.0),
            1.0,
            &tuning.clearance,
        );

        assert!(kick.momentum.x > 0.0, "despeja hacia el campo contrario");
        assert!(kick.momentum.y.abs() > 0.0, "abre hacia una banda");
        assert!(kick.momentum.z > 0.0, "la levanta");
    }

    /// El límite del modelo heredado: un defensa que corre hacia su propia
    /// portería despeja HACIA ella. Se afirma tal cual para que el día que se
    /// arregle, este test cambie a propósito y no por sorpresa.
    #[test]
    fn a_clearance_while_retreating_still_goes_backwards_today() {
        let tuning = MatchTuning::default();
        let kick = solve_clearance(
            Vec2::new(-45.0, 2.0),
            Vec2::new(-3.0, 0.0),
            1.0,
            &tuning.clearance,
        );

        assert!(
            kick.momentum.x < 0.0,
            "si esto pasa a ser positivo, el modelo mejoró: actualiza el test"
        );
    }

    /// En tráfico el toque se acorta, o el balón rueda fuera del alcance del
    /// portador y se lo queda el rival.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "la receta devuelve estas cifras exactas, y el test las repite"
    )]
    fn a_knock_on_shortens_in_traffic() {
        let tuning = ContestTuning::default();
        let running = Vec2::new(7.0, 0.0);
        let open = solve_knock_on(Vec2::ZERO, running, Vec2::X, 20.0, &tuning);
        let crowded = solve_knock_on(Vec2::ZERO, running, Vec2::X, 1.0, &tuning);

        assert!(crowded.momentum.length() < open.momentum.length());
        assert_eq!(crowded.momentum.length(), tuning.carry_pace_in_traffic);
    }

    /// El balón se queda en el pie: un toque nunca sale más rápido que quien lo
    /// da, o deja de ser conducción y pasa a ser un pase a nadie.
    #[test]
    fn a_knock_on_never_outruns_the_carrier() {
        let tuning = ContestTuning::default();

        for pace in [2.0_f32, 4.0, 6.0, 8.0] {
            let touch = solve_knock_on(Vec2::ZERO, Vec2::new(pace, 0.0), Vec2::X, 20.0, &tuning);
            assert!(
                touch.momentum.length() <= pace.max(1.5),
                "corriendo a {pace} m/s el toque salió a {}",
                touch.momentum.length()
            );
        }
    }
}
