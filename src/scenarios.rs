//! Catalogue of reproducible situations.
//!
//! Each one states an initial state, a window and what must happen, so a rule is
//! demonstrated rather than asserted. The rule-of-play scenarios deliberately
//! field no players: an incident that depends on nobody's behaviour is the
//! smallest thing that can prove a referee decision.

use bevy::math::Vec3;
use football_domain::scenario::{BallSetup, Expectations, PlayerSetup};
use football_domain::{ByTeam, MatchPhase, MatchRegulations, Scenario, SetPiece, TeamId};
use std::time::Duration;

/// A full match from the opening whistle, both teams formed up.
pub fn kick_off() -> Scenario {
    Scenario::kick_off()
}

/// Los diez minutos que de verdad se miran.
///
/// Mientras los cuerpos sean cápsulas sin motor ni atributos, noventa minutos
/// no dicen más que diez: la misma unidad que mide `comparing_builds`, para que
/// mirar y medir sean la misma situación. El partido reglamentario vuelve a ser
/// el escenario de la app cuando haya jugadores que lo llenen.
pub fn lab_match() -> Scenario {
    Scenario::kick_off()
        .named("lab match")
        .for_duration(Duration::from_secs(10 * 60))
}

/// The first minutes of open play. Same situation as [`kick_off`], short enough
/// to run in a test.
/// El arranque del partido, y la ventana que corre media suite: ocho tests lo
/// usan, uno de ellos con dos runners. Cada segundo de aquí son ocho de reloj.
pub fn opening_minute() -> Scenario {
    Scenario::kick_off()
        .named("opening minute")
        .for_duration(Duration::from_secs(20))
}

/// Law 10: the whole ball passes over the goal line between the posts and under
/// the crossbar, so a goal is awarded and play restarts with a kick-off.
pub fn shot_crossing_the_goal_line() -> Scenario {
    Scenario::kick_off()
        .named("shot crossing the goal line")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(50.0, 0.0, 0.6), Vec3::new(30.0, 0.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        // 8 s of celebration plus the restart, per the original's timings
        .for_duration(Duration::from_secs(12))
        .expecting(Expectations {
            score: Some(ByTeam::new(1, 0)),
            set_pieces: vec![SetPiece::KickOff],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 9: the whole ball crosses the touchline, so the throw-in goes to the
/// team that did not touch it last.
pub fn ball_over_the_touchline() -> Scenario {
    Scenario::kick_off()
        .named("ball over the touchline")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(0.0, 34.0, 0.11), Vec3::new(0.0, 12.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            set_pieces: vec![SetPiece::ThrowIn],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 16: home plays the ball over the opponents' goal line outside the goal,
/// so the defending side gets a goal kick.
pub fn ball_over_the_opponents_goal_line() -> Scenario {
    Scenario::kick_off()
        .named("ball over the opponents' goal line")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(52.0, 20.0, 0.11), Vec3::new(12.0, 0.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            set_pieces: vec![SetPiece::GoalKick],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 10, the near miss: a shot that crosses the goal line half a metre wide
/// of the post is not a goal, however close it looked. This is the outside of
/// the side netting, and the only thing separating it from
/// [`shot_crossing_the_goal_line`] is where along y it crossed.
pub fn shot_into_the_side_netting() -> Scenario {
    Scenario::kick_off()
        .named("shot into the side netting")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            // 4.2 m off centre: outside the 3.7 m post by half a metre, well
            // clear of the post's own radius, so nothing here is a woodwork
            // rebound in disguise.
            BallSetup::travelling_from(Vec3::new(50.0, 4.2, 0.6), Vec3::new(30.0, 0.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            set_pieces: vec![SetPiece::GoalKick],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 17: home puts the ball over its own goal line outside the goal, so the
/// attacking side gets a corner.
pub fn ball_over_own_goal_line() -> Scenario {
    Scenario::kick_off()
        .named("ball over own goal line")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(-52.0, 20.0, 0.11), Vec3::new(-12.0, 0.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(8))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            set_pieces: vec![SetPiece::Corner],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 7: a whole match, from before kick-off to full time, with the periods
/// shortened to seconds.
///
/// Shortening them is the point of holding regulation lengths as competition
/// data: the phases are the same ones a ninety-minute match goes through.
pub fn short_match() -> Scenario {
    Scenario::kick_off()
        .named("short match")
        .with_regulations(MatchRegulations {
            half_duration: Duration::from_secs(20),
            half_time_interval: Duration::from_secs(3),
            ..MatchRegulations::default()
        })
        .for_duration(Duration::from_secs(50))
        .expecting(Expectations {
            phases: vec![
                MatchPhase::FirstHalf,
                MatchPhase::HalfTime,
                MatchPhase::SecondHalf,
                MatchPhase::FullTime,
            ],
            play_resumes: true,
            ..Default::default()
        })
}

/// Ley 3: un cambio pedido por la situación se sirve en la primera detención,
/// sin alterar cuerpos mientras el balón está vivo.
pub fn substitution_at_kick_off() -> Scenario {
    Scenario::kick_off()
        .named("substitution at kick-off")
        .with_substitutions(vec![football_domain::Substitution::new(
            football_domain::PlayerId::home(9),
            football_domain::PlayerId::home(19),
        )])
        .for_duration(Duration::from_secs(4))
        .expecting(Expectations {
            play_resumes: true,
            ..Default::default()
        })
}

/// Leyes 7 y 10: si un empate eliminatorio sobrevive a las dos prórrogas, la
/// tanda es una fase propia antes del final, no goles escondidos en el marcador.
pub fn drawn_match_to_penalties() -> Scenario {
    Scenario::kick_off()
        .named("drawn match to penalties")
        .with_regulations(MatchRegulations {
            half_duration: Duration::from_secs(1),
            half_time_interval: Duration::ZERO,
            extra_time_half_duration: Some(Duration::from_secs(1)),
            extra_time_interval: Duration::ZERO,
            kicks_from_penalty_mark_if_draw: true,
            shootout_conversion_probability: 0.63,
            ..MatchRegulations::default()
        })
        .for_duration(Duration::from_secs(15))
        .expecting(Expectations {
            phases: vec![
                MatchPhase::FirstExtraTime,
                MatchPhase::SecondExtraTime,
                MatchPhase::Penalties,
                MatchPhase::FullTime,
            ],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 9 at speed: a shot travelling 0.4 m per tick must not tunnel through the
/// goal line between two ticks. The referee sweeps the segment for this reason.
pub fn goal_at_high_speed() -> Scenario {
    Scenario::kick_off()
        .named("goal at high speed")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(45.0, 1.0, 1.0), Vec3::new(40.0, 0.5, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(12))
        .expecting(Expectations {
            score: Some(ByTeam::new(1, 0)),
            set_pieces: vec![SetPiece::KickOff],
            play_resumes: true,
            ..Default::default()
        })
}

/// Law 10: hitting the crossbar is not a goal, however close it looks.
pub fn shot_off_the_crossbar() -> Scenario {
    Scenario::kick_off()
        .named("shot off the crossbar")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(50.0, 0.0, 2.4), Vec3::new(22.0, 0.0, 1.5))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(6))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            ..Default::default()
        })
}

/// Law 10: hitting the post is not a goal either — the ball comes back into
/// play instead.
///
/// The shot is aimed straight down the axis of the post, which is the only
/// aim that is unambiguously woodwork: a hand's breadth to either side and the
/// question becomes whether the ball was inside or outside the post, which is a
/// different claim (and the one `shot_crossing_the_goal_line` already makes).
pub fn shot_off_the_post() -> Scenario {
    Scenario::kick_off()
        .named("shot off the post")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(52.0, 3.7, 0.5), Vec3::new(10.0, 0.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(6))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            play_never_stops: true,
            ..Default::default()
        })
}

/// Law 9 and 10: the ball must pass WHOLLY over the line. A ball that stops on
/// the line is neither a goal nor out of play — it is still live.
pub fn ball_stopping_on_the_goal_line() -> Scenario {
    Scenario::kick_off()
        .named("ball stopping on the goal line")
        .with_players(PlayerSetup::BallOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(54.9, 0.0, 0.11), Vec3::new(0.7, 0.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(6))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            play_never_stops: true,
            ..Default::default()
        })
}

/// El portero para lo que alcanza: un disparo raso y centrado, con el tiempo de
/// vuelo suficiente para que se llegue a tirar.
///
/// Aísla el mecanismo: solo los dos porteros en el campo, así que lo que pase
/// con el balón no lo puede haber hecho nadie más.
pub fn shot_saved_by_the_keeper() -> Scenario {
    Scenario::kick_off()
        .named("shot saved by the keeper")
        .with_players(PlayerSetup::GoalkeepersOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(-30.0, 1.5, 0.4), Vec3::new(-18.0, 0.0, 0.0))
                .last_touched_by(TeamId::Away),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(6))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 0)),
            ..Default::default()
        })
}

/// Y encaja lo que no alcanza: pegado al poste y a dos décimas de la línea.
///
/// Va en pareja con [`shot_saved_by_the_keeper`] a propósito. Un portero que
/// parase todo cumpliría el primero, y solo este dice que lo que ataja es lo
/// que alcanza. La diferencia entre los dos es el tiempo: aquí no le da para
/// desplazarse, y lo que cubre desde donde está no llega al palo.
pub fn shot_beyond_the_keepers_reach() -> Scenario {
    Scenario::kick_off()
        .named("shot beyond the keeper's reach")
        .with_players(PlayerSetup::GoalkeepersOnly)
        .with_ball(
            BallSetup::travelling_from(Vec3::new(-48.0, 3.3, 0.3), Vec3::new(-35.0, 0.0, 0.0))
                .last_touched_by(TeamId::Away),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(12))
        .expecting(Expectations {
            score: Some(ByTeam::new(0, 1)),
            set_pieces: vec![SetPiece::KickOff],
            ..Default::default()
        })
}

/// Ley 7: una parte con una interrupción, para que el añadido tenga qué añadir.
///
/// El balón sale por la banda al segundo de empezar y nadie lo devuelve al
/// juego —no hay jugadores—, así que la parada es una, dura lo que dura la
/// reanudación y se puede comparar contra el reloj sin depender del azar.
pub fn interrupted_half() -> Scenario {
    Scenario::kick_off()
        .named("interrupted half")
        .with_players(PlayerSetup::BallOnly)
        .with_regulations(MatchRegulations {
            half_duration: Duration::from_secs(10),
            half_time_interval: Duration::from_secs(2),
            ..MatchRegulations::default()
        })
        .with_ball(
            BallSetup::travelling_from(Vec3::new(0.0, 30.0, 0.11), Vec3::new(0.0, 12.0, 0.0))
                .last_touched_by(TeamId::Home),
        )
        .already_in_play()
        .for_duration(Duration::from_secs(30))
        .expecting(Expectations {
            set_pieces: vec![SetPiece::ThrowIn],
            phases: vec![MatchPhase::FirstHalf, MatchPhase::HalfTime],
            ..Default::default()
        })
}

/// Every scenario in the catalogue, for suites that run them all.
pub fn all() -> Vec<Scenario> {
    vec![
        opening_minute(),
        short_match(),
        substitution_at_kick_off(),
        drawn_match_to_penalties(),
        shot_crossing_the_goal_line(),
        goal_at_high_speed(),
        shot_off_the_crossbar(),
        shot_off_the_post(),
        ball_stopping_on_the_goal_line(),
        ball_over_the_touchline(),
        ball_over_the_opponents_goal_line(),
        ball_over_own_goal_line(),
        shot_into_the_side_netting(),
        shot_saved_by_the_keeper(),
        shot_beyond_the_keepers_reach(),
        interrupted_half(),
    ]
}
