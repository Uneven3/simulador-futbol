//! The parameters that fix the result of a match, as versioned data: a number
//! that decides whether a shot is taken is the model, not an implementation
//! detail, and it has to be sweepable and reportable (§8, `VALIDATION.md`).
//!
//! One home per default — a value here appears nowhere else — and every name
//! carries its unit. Still outside: the movement velocities and force field
//! weights (motor model, MVP 3) and the ball physics constants, which are
//! calibrated against ball flight rather than match statistics.

use bevy_ecs::prelude::*;
use std::time::Duration;

/// Which calibration produced a result.
///
/// A goal rate means nothing without the parameters behind it, so every run can
/// name them. `MatchRegulations` records the edition of the Laws for the same
/// reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TuningVersion {
    /// The envelope inherited from the C++ original, never calibrated against
    /// anything: 51 goals per 90 minutes against ~2.7 real ones
    /// (`docs/VALIDATION.md`). It is the starting point of
    /// MVP 1.75, not a defensible model.
    PortBaseline,
}

/// Everything that decides how a match turns out, in one place.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct MatchTuning {
    pub version: TuningVersion,
    pub contest: ContestTuning,
    pub possession: PossessionTuning,
    pub passing: PassingTuning,
    pub clearance: ClearanceTuning,
    pub shooting: ShootingTuning,
    pub striking: StrikingTuning,
    pub stamina: StaminaTuning,
    pub turning: TurningTuning,
    pub defending: DefendingTuning,
    pub goalkeeping: GoalkeepingTuning,
    pub refereeing: RefereeTuning,
}

impl Default for MatchTuning {
    fn default() -> Self {
        Self {
            version: TuningVersion::PortBaseline,
            contest: ContestTuning::default(),
            possession: PossessionTuning::default(),
            passing: PassingTuning::default(),
            clearance: ClearanceTuning::default(),
            shooting: ShootingTuning::default(),
            striking: StrikingTuning::default(),
            stamina: StaminaTuning::default(),
            turning: TurningTuning::default(),
            defending: DefendingTuning::default(),
            goalkeeping: GoalkeepingTuning::default(),
            refereeing: RefereeTuning::default(),
        }
    }
}

/// Lo que tarda un golpeo desde que se decide hasta que el pie llega al balón.
///
/// Es el único sitio donde el fútbol de este simulador ocurre en el futuro:
/// mientras dura, el rival sigue jugando y el compañero al que va el pase sigue
/// corriendo, así que comprometerse cuesta algo. Un pase corto se arma en dos
/// décimas y un disparo con carrera necesita más del doble.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrikingTuning {
    pub pass_windup: Duration,
    pub shot_windup: Duration,
    pub clearance_windup: Duration,
    /// A qué paso se ajusta al balón mientras arma la pierna (m/s). Quien sigue
    /// corriendo a su ritmo se deja el balón atrás y no llega a golpearlo, que
    /// es lo que pasaba cuando esto no existía: cero tiros en diez minutos.
    pub adjust_pace: f32,
}

impl Default for StrikingTuning {
    fn default() -> Self {
        Self {
            pass_windup: Duration::from_millis(200),
            shot_windup: Duration::from_millis(350),
            clearance_windup: Duration::from_millis(250),
            adjust_pace: 2.0,
        }
    }
}

/// Lo que cuesta correr hacia donde no se mira.
///
/// Un futbolista de espaldas cubre alrededor del sesenta por ciento de lo que
/// cubre de frente, y de lado algo más; girar el cuerpo a la carrera es más
/// lento que girarlo parado, porque las piernas están ocupadas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurningTuning {
    /// Fracción de la velocidad que se alcanza corriendo hacia atrás...
    pub backpedal_pace: f32,
    /// ...y de lado, a noventa grados de donde se mira.
    pub sideways_pace: f32,
    /// Fracción del giro que queda a velocidad punta: girar corriendo cuesta.
    pub turn_at_speed: f32,
}

impl Default for TurningTuning {
    fn default() -> Self {
        Self {
            backpedal_pace: 0.6,
            sideways_pace: 0.8,
            turn_at_speed: 0.4,
        }
    }
}

/// Lo que cuesta correr y lo que se recupera andando.
///
/// La referencia es el partido real: un futbolista cubre diez u once
/// kilómetros en noventa minutos casi todos al trote, y lo que no aguanta es el
/// sprint —del orden de medio minuto acumulado antes de tener que bajar—. De
/// ahí que el trote no gaste nada y la punta vacíe el depósito en cuarenta
/// segundos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaminaTuning {
    /// Fracción del depósito que cuesta un segundo a velocidad punta.
    pub sprint_drain: f32,
    /// ...y la que se recupera por segundo por debajo del trote.
    pub recovery: f32,
    /// Por debajo de esta velocidad (m/s) se recupera en vez de gastar.
    pub recovery_pace: f32,
    /// A qué fracción de su punta corre un jugador vacío...
    pub spent_speed: f32,
    /// ...y con qué fracción de su aceleración arranca, que cae más: lo primero
    /// que se pierde cansado es el arranque, no la punta.
    pub spent_acceleration: f32,
}

impl Default for StaminaTuning {
    fn default() -> Self {
        Self {
            sprint_drain: 0.025,
            recovery: 0.012,
            recovery_pace: 3.0,
            spent_speed: 0.8,
            spent_acceleration: 0.65,
        }
    }
}

/// Lo que el árbitro decide, más allá de lo que la ley fija.
#[derive(Debug, Clone, PartialEq)]
pub struct RefereeTuning {
    /// Cuánto espera antes de dar por buena una ventaja (Ley 5). La ley dice
    /// "unos segundos": si en ese tiempo el equipo infringido conserva el balón,
    /// la falta no se pita; si lo pierde, se vuelve a ella.
    pub advantage_window: Duration,
    /// Si el árbitro detiene el juego por las faltas que observa.
    pub whistles_fouls: bool,
}

impl Default for RefereeTuning {
    fn default() -> Self {
        Self {
            advantage_window: Duration::from_secs(3),
            whistles_fouls: true,
        }
    }
}

/// Winning, keeping and losing the ball: the contact distances and the cadences
/// that decide who has it. Most of these exist because this port has no
/// animation layer — the original shields, traps and tackles with body
/// animations, and these windows stand in for them.
#[derive(Debug, Clone, PartialEq)]
pub struct ContestTuning {
    /// Beyond this distance from the carrier (m) the ball counts as escaped and
    /// possession is loose again.
    pub possession_escape_distance: f32,
    /// Contact distance to pick up a loose ball (m).
    pub loose_ball_reach: f32,
    /// The intended receiver of a pass reaches further (m): stand-in for the
    /// original's trap animations stretching a leg.
    pub receiver_trap_reach: f32,
    /// How much the receiver's reach wins ties inside his radius (m).
    pub receiver_tie_break: f32,
    /// A ball higher than this cannot be taken on the ground (m).
    pub max_touch_height: f32,
    /// Contact distance for a deliberate tackle (m).
    pub tackle_contact_distance: f32,
    /// Cuerpos más cerca que esto (m) durante una entrada fallida es contacto
    /// con el hombre, y eso es falta. Los cuerpos se separan a 0,7 m, así que
    /// por debajo de eso están literalmente encima.
    pub foul_contact_distance: f32,
    /// Y va al hombre esto (m/s) más de lo que va al balón: lo que separa
    /// entrar de disputar. Perseguir a quien lleva el balón en el pie apunta a
    /// los dos a la vez y no descuenta nada; entrar tarde es la carrera que
    /// sigue yendo al cuerpo cuando el balón ya no está ahí.
    pub foul_charge: f32,
    /// The tackler must be this much closer to the ball than the carrier
    /// (fraction of the carrier's distance) to win the duel.
    pub duel_advantage: f32,
    /// Por encima de esta velocidad (m/s) el balón ya no se disputa: viaja, y
    /// cualquier cuerpo en su camino lo desvía —incluido el designado, que en
    /// una disputa sí se salta el choque—.
    pub travelling_ball_speed: f32,
    /// The ball is stealable when it is at least this far from the carrier's
    /// feet (m) — poked loose rather than under control.
    pub shielding_release_distance: f32,
    /// ...or when he has held it this long, and pressure legitimately forces the
    /// turnover.
    pub shielding_release_time: Duration,
    /// A carrier cannot be tackled until he has held the ball this long.
    pub steal_cooldown: Duration,
    /// A player who just lost the ball waits longer to win it back, or the two
    /// designated players trade it every window.
    pub regain_cooldown: Duration,
    /// Nobody picks up a loose ball this soon after any kick.
    pub loose_ball_cooldown: Duration,
    /// The last toucher waits longer: the ball has to leave his feet.
    pub own_ball_cooldown: Duration,
    /// How long the last toucher keeps priority in a shoulder-to-shoulder race
    /// (original `GetLastTouchBias`).
    pub touch_bias_window: Duration,
    /// The bias only applies while he is this close to the ball (m)...
    pub touch_bias_distance: f32,
    /// ...and an opponent must arrive this much closer (m) to overrule it.
    pub touch_bias_margin: f32,
    /// The ball is at the carrier's feet within this distance (m)...
    pub ball_at_feet_distance: f32,
    /// ...and below this height (m).
    pub ball_at_feet_height: f32,
    /// Reaction time before a deliberate release (pass, shot, clearance).
    pub decision_cadence: Duration,
    /// Slower cadence between dribble knock-ons.
    pub knock_on_cadence: Duration,
    /// A knock-on with an opponent this close (m) is shortened to dribble pace.
    pub knock_on_traffic_distance: f32,
    /// A controlled first touch sets the ball up at this fraction of the
    /// carrier's speed...
    pub trap_speed_from_run: f32,
    /// ...clamped to this range (m/s). A dead trap parks the ball in the middle
    /// of the duel and invites the steal.
    pub trap_speed_range: (f32, f32),
}

impl Default for ContestTuning {
    fn default() -> Self {
        Self {
            possession_escape_distance: 3.0,
            loose_ball_reach: 0.65,
            receiver_trap_reach: 1.1,
            receiver_tie_break: 0.45,
            max_touch_height: 1.5,
            tackle_contact_distance: 0.50,
            foul_contact_distance: 0.75,
            foul_charge: 2.0,
            duel_advantage: 0.8,
            travelling_ball_speed: 8.0,
            shielding_release_distance: 1.0,
            shielding_release_time: Duration::from_millis(2000),
            steal_cooldown: Duration::from_millis(500),
            regain_cooldown: Duration::from_millis(1000),
            loose_ball_cooldown: Duration::from_millis(220),
            own_ball_cooldown: Duration::from_millis(400),
            touch_bias_window: Duration::from_millis(1500),
            touch_bias_distance: 1.0,
            touch_bias_margin: 0.25,
            ball_at_feet_distance: 0.7,
            ball_at_feet_height: 0.5,
            decision_cadence: Duration::from_millis(150),
            knock_on_cadence: Duration::from_millis(350),
            knock_on_traffic_distance: 3.0,
            trap_speed_from_run: 0.5,
            trap_speed_range: (2.0, 3.5),
        }
    }
}

/// How a player judges whether the ball is his to go for (the magnet branch of
/// the original's `_MovementCommand`).
#[derive(Debug, Clone, PartialEq)]
pub struct PossessionTuning {
    /// `possessionAmount` above which we would beat everyone to the ball, so the
    /// designated player goes for it whatever the state of play.
    pub winnable_outright: f32,
    /// ...and above which a loose ball is worth chasing.
    pub winnable_loose: f32,
    /// Softening term (ms) in `possessionAmount = (oppTime + e) / (myTime + e)`,
    /// so two players arriving at once do not divide by zero.
    pub time_to_ball_softening_ms: f32,
    /// Times beyond this (ms) are all equally hopeless.
    pub time_to_ball_cap_ms: f32,
}

impl Default for PossessionTuning {
    fn default() -> Self {
        Self {
            winnable_outright: 0.99,
            winnable_loose: 0.5,
            time_to_ball_softening_ms: 200.0,
            time_to_ball_cap_ms: 60_000.0,
        }
    }
}

/// When a pass is worth attempting, and to whom.
#[derive(Debug, Clone, PartialEq)]
pub struct PassingTuning {
    /// A teammate must improve on the carrier's tactical rating by at least this
    /// much (rating, 0..1) — scaled down for attacking mindsets.
    pub tactical_improvement_threshold: f32,
    /// Passing back to the goalkeeper is rated at this fraction of its odds.
    pub keeper_target_penalty: f32,
    /// Minimum odds (0..1) to attempt a pass at all, plus what a defensive
    /// mindset adds and what a long spell of possession takes off. This sits
    /// 0.15 above the original: with no `AI_GetPass` refinement at the touch
    /// moment and no receiver trap animations, more 50/50 deliveries are lost.
    pub minimum_odds: f32,
    pub minimum_odds_defensive_bonus: f32,
    pub minimum_odds_long_possession_relief: f32,
    /// Minimum combined rating (tactical gain + odds) to prefer the pass, and
    /// what a long spell of possession takes off it.
    pub combined_threshold: f32,
    pub combined_threshold_long_possession_relief: f32,
    /// How long a carrier must hold the ball (ms) for "long possession" to be
    /// fully in effect.
    pub long_possession_ms: f32,
    /// Hemmed in: this many opponents within this distance (m) and dribbling on
    /// only feeds the herd-out towards the sideline.
    pub hemmed_opponents: usize,
    pub hemmed_distance: f32,
    /// Minimum odds for the escape pass out of a hemmed-in carrier, below which
    /// he blasts it clear.
    pub escape_minimum_odds: f32,
    /// A receiver this far past the offside line (m) is a wasted touch.
    pub offside_receiver_margin: f32,
    /// A high pass shorter than this (m) makes no sense.
    pub high_pass_min_distance: f32,
    /// Danger added to every high pass: it hangs, and hanging is readable.
    pub high_pass_danger: f32,
    /// Vertical launch fraction per pass kind (dimensionless, per
    /// `AI_GetAutoPass`), and the pace multiplier that makes the pass ARRIVE
    /// instead of dying at the receiver's feet.
    pub short_lift: f32,
    pub short_pace: f32,
    pub long_lift: f32,
    pub long_pace: f32,
    pub high_lift: f32,
    /// How much of the high lift is traded away over 60 m of range.
    pub high_lift_range_relief: f32,
    pub high_pace: f32,
}

impl Default for PassingTuning {
    fn default() -> Self {
        Self {
            tactical_improvement_threshold: 0.06,
            keeper_target_penalty: 0.7,
            minimum_odds: 0.15,
            minimum_odds_defensive_bonus: 0.2,
            minimum_odds_long_possession_relief: 0.1,
            combined_threshold: 0.1,
            combined_threshold_long_possession_relief: 0.05,
            long_possession_ms: 5000.0,
            hemmed_opponents: 2,
            hemmed_distance: 3.0,
            escape_minimum_odds: 0.2,
            offside_receiver_margin: 0.5,
            high_pass_min_distance: 10.0,
            high_pass_danger: 0.4,
            short_lift: 0.11,
            short_pace: 1.5,
            long_lift: 0.14,
            long_pace: 2.0,
            high_lift: 0.45,
            high_lift_range_relief: 0.15,
            high_pace: 1.5,
        }
    }
}

/// Panic: hoofing the ball away instead of playing it (`_AddPanicPass`).
///
/// Not a goalkeeper matter — any defensive player close to his own goal with no
/// pass on does it, and how readily he does it decides how often possession is
/// simply given back.
#[derive(Debug, Clone, PartialEq)]
pub struct ClearanceTuning {
    /// Only players below this attacking bias (0..1) panic at all.
    pub defensive_mindset_max: f32,
    /// Closeness to his own goal is measured between these distances (m): at the
    /// near one he is fully under threat, at the far one not at all.
    pub goal_closeness_near: f32,
    pub goal_closeness_far: f32,
    /// `possessionAmount` below which he panics: this base, plus a share of how
    /// threatened he is.
    pub possession_threshold: f32,
    pub possession_threshold_gain: f32,
    /// Speed of the clearance (m/s) and its vertical launch fraction.
    pub power: f32,
    pub lift: f32,
}

impl Default for ClearanceTuning {
    fn default() -> Self {
        Self {
            defensive_mindset_max: 0.25,
            goal_closeness_near: 2.0,
            goal_closeness_far: 16.0,
            possession_threshold: 0.9,
            possession_threshold_gain: 0.8,
            power: 17.0,
            lift: 0.3,
        }
    }
}

/// When a shot is taken, and with what.
///
/// The first suspect of MVP 1.75: the gate below lets a player shoot from
/// almost anywhere, and shots that reach the goal are rarely stopped.
#[derive(Debug, Clone, PartialEq)]
pub struct ShootingTuning {
    /// The ideal shooting spot sits this far in front of the goal line (m)...
    pub ideal_position_offset: f32,
    /// ...and the position factor fades to zero this far from it (m).
    pub ideal_position_range: f32,
    /// Below this position factor (0..1) shooting is not even considered.
    pub ideal_position_gate: f32,
    /// Shot odds (0..1) plus a random draw in `0..odds_random_span` must clear
    /// this to pull the trigger.
    pub odds_threshold: f32,
    pub odds_random_span: f32,
    /// The shot is rated as a pass travelling this many times faster.
    pub odds_velocity_multiplier: f32,
    /// Where the aim is probed on the goal line (m from centre) and where the
    /// ball is actually aimed once a side is chosen.
    pub aim_probe_y: f32,
    pub aim_y: f32,
    /// Aim is dragged towards the centre by this factor before the technique
    /// spread is applied.
    pub aim_centre_pull: f32,
    /// Kick power (m/s): a random draw in this range, scaled by distance, plus a
    /// floor.
    pub power_random_range: (f32, f32),
    pub power_distance_base: f32,
    pub power_distance_gain: f32,
    pub power_scale: f32,
    pub power_floor: f32,
    /// Distance to the goal (m) at which the power and lift factors saturate.
    pub power_distance_range: f32,
    /// Vertical launch fraction, plus what distance adds to it.
    pub lift: f32,
    pub lift_distance_gain: f32,
    /// Topspin and sidespin applied to the strike.
    pub topspin: f32,
    pub sidespin: f32,
}

impl Default for ShootingTuning {
    fn default() -> Self {
        Self {
            ideal_position_offset: 7.0,
            ideal_position_range: 16.0,
            ideal_position_gate: 0.1,
            odds_threshold: 0.5,
            odds_random_span: 0.5,
            odds_velocity_multiplier: 3.0,
            aim_probe_y: 3.6,
            aim_y: 3.5,
            aim_centre_pull: 0.8,
            power_random_range: (0.75, 1.0),
            power_distance_base: 0.6,
            power_distance_gain: 0.4,
            power_scale: 24.0,
            power_floor: 6.0,
            power_distance_range: 32.0,
            lift: 0.05,
            lift_distance_gain: 0.05,
            topspin: 12.0,
            sidespin: 8.0,
        }
    }
}

/// Off-the-ball defending: when to leave shape and how far to cover.
#[derive(Debug, Clone, PartialEq)]
pub struct DefendingTuning {
    /// How close to the goal (m) the ball carrier is treated as a shooting
    /// threat, and how close anyone else has to be.
    pub carrier_threat_distance: f32,
    pub generic_threat_distance: f32,
    /// The covering point never collapses onto the opponent (m)...
    pub cover_min_distance: f32,
    /// ...and a defender who is this much better placed (m) does not bother
    /// closing further.
    pub cover_buffer_distance: f32,
    /// Hunting range for the carrier (m): the base, plus what a defensive
    /// mindset adds.
    pub hunt_distance: f32,
    pub hunt_distance_defensive_bonus: f32,
}

impl Default for DefendingTuning {
    fn default() -> Self {
        Self {
            carrier_threat_distance: 24.0,
            generic_threat_distance: 8.0,
            cover_min_distance: 0.4,
            cover_buffer_distance: 4.0,
            hunt_distance: 10.0,
            hunt_distance_defensive_bonus: 10.0,
        }
    }
}

/// The goalkeeper: where he stands, when he comes out, and when he hoofs it.
///
/// The second suspect of MVP 1.75: "the keeper does not really defend".
#[derive(Debug, Clone, PartialEq)]
pub struct GoalkeepingTuning {
    /// Resting distance in front of the goal line (m).
    pub line_distance: f32,
    /// How far ahead he reads the ball (ms): a base plus a share of his own time
    /// to reach it.
    pub prediction_base_ms: f32,
    pub prediction_time_share: f32,
    /// The area he will come out to collect a loose ball in: this far from the
    /// goal line (m) and this far from the centre (m).
    pub box_depth: f32,
    pub box_half_width: f32,
    /// A ball counts as collectable below this speed (m/s) and this height (m),
    /// within this distance of him (m), with the nearest opponent at least this
    /// far away in time (ms).
    pub collectable_speed: f32,
    pub collectable_height: f32,
    pub collect_distance: f32,
    pub collect_opponent_margin_ms: f32,
    /// Distance from goal (m) at which an onrushing opponent is treated as
    /// about to shoot, for the keeper himself and for the opponent's helper.
    pub come_out_threat_distance: f32,
    pub helper_threat_distance: f32,
    /// How much wider than the posts a ball still counts as bound for goal
    /// (factor on the goal half-width, with average keeper reading).
    pub panic_factor: f32,
    /// He only reads a shot as bound for goal from within this distance (m).
    pub bound_for_goal_range: f32,
    /// `possessionAmount` below which the keeper just clears his lines instead
    /// of playing. At 3.0 he clears in nearly every situation.
    pub clearance_possession_threshold: f32,
    /// He plays the ball only once it is this close to him in time (s): until
    /// then he is moving, not saving.
    pub reaction_window: f32,
    /// How far he gets to either side of himself, arms out (m). It does not grow
    /// with the time available — getting there is what `goalie_movement` does —
    /// so what beats him is a ball passing wide of wherever he managed to be.
    /// Standing ten metres off his line he already covers most of the angle, so
    /// this is a body and an arm, not a leap across the goal.
    pub dive_reach: f32,
    /// How high he gets a hand to (m), standing or airborne.
    pub reach_height: f32,
    /// He catches a ball arriving below this speed (m/s) and parries the rest,
    /// which leaves at this share of the pace it came in with.
    pub catchable_speed: f32,
    pub parry_pace: f32,
}

impl Default for GoalkeepingTuning {
    fn default() -> Self {
        Self {
            line_distance: 10.0,
            prediction_base_ms: 600.0,
            prediction_time_share: 0.2,
            box_depth: 16.4,
            box_half_width: 20.0,
            collectable_speed: 4.0,
            collectable_height: 1.5,
            collect_distance: 8.0,
            collect_opponent_margin_ms: 400.0,
            come_out_threat_distance: 20.0,
            helper_threat_distance: 24.0,
            panic_factor: 1.02,
            bound_for_goal_range: 32.0,
            clearance_possession_threshold: 3.0,
            reaction_window: 0.05,
            dive_reach: 1.4,
            reach_height: 2.4,
            catchable_speed: 12.0,
            parry_pace: 0.45,
        }
    }
}
