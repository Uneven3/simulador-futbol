//! What one player decides to do this tick: where to run, and — if he has the
//! ball — what to do with it.
//!
//! It decides and nothing else: the ball is touched in [`crate::ball_contest`]
//! and [`crate::ball_release`].
//!
//! Ported from `ElizaController`
//! (onthepitch/player/controller/elizacontroller.cpp), the off-the-ball
//! strategies (strategies/offtheball/default_def|mid|off.cpp) and the keeper
//! (goalie_default.cpp).
//!
//! Without an animation layer, the original's `PlayerCommand`s become a per-tick
//! `Velocity` and an `OnBallAction`. Deciders still read the true world, not an
//! observed one: the `MentalImage` delay arrives with MVP 4.

use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use std::time::Duration;

use crate::force_field::{self, Falloff, ForceSpot};
use crate::team_tactics::{
    DISTANCE_TO_VELOCITY_MULTIPLIER, DRIBBLE_VELOCITY, PITCH_HALF_H, PITCH_HALF_W, PlayerReading,
    SPRINT_VELOCITY, TeamTactics, WALK_VELOCITY, apply_offside_trap, closest_player,
    closest_players, cpp_clamp, get_adapted_formation_position,
};
use football_domain::math::{
    curve, line_distance_to_point_2d, line_intersection_2d, normalized_clamp, normalized_or_2d,
    rotated_2d, sample_index,
};
use football_domain::tuning::{GoalkeepingTuning, PassingTuning};
use football_domain::{
    Attributes, Ball, MatchRng, MatchState, MatchTuning, Mentality, PitchSides, Player, PlayerId,
    PlayerMatchState, PlayingPosition, Position, PossessionDesignation, SetPiece, TeamId, Velocity,
};

// ---------------------------------------------------------------------------
// Shared context assembled once per tick
// ---------------------------------------------------------------------------

struct DecisionContext<'a> {
    snaps: &'a [PlayerReading],
    /// Qué mitad defiende cada equipo ahora: toda la geometría cuelga de esto.
    sides: PitchSides,
    ball: &'a Ball,
    tactics: &'a TeamTactics,
    tuning: &'a MatchTuning,
    designation: &'a PossessionDesignation,
    /// The single player (either team) expected to reach the ball first
    /// (original `Match::GetDesignatedPossessionPlayer`).
    match_designated: Option<PlayerId>,
    now_ms: u64,
}

fn snap_of(snaps: &[PlayerReading], id: PlayerId) -> Option<&PlayerReading> {
    snaps.iter().find(|s| s.id == id)
}

/// Offside line faced by attackers of `att_team`: one-but-deepest opponent
/// (projected `future_ms` ahead) or the ball, never inside the attackers' own
/// half (port of `AI_GetOffsideLine`).
pub fn offside_line(
    snaps: &[PlayerReading],
    att_team: TeamId,
    sides: PitchSides,
    ball_x: f32,
    future_ms: f32,
) -> f32 {
    let def_team = att_team.opponent();
    let def_side = sides.defending_x(def_team);
    let projected: Vec<f32> = snaps
        .iter()
        .filter(|s| s.team() == def_team)
        .map(|s| s.pos.x + s.vel.x * future_ms * 0.001)
        .collect();

    let mut deepest_idx: Option<usize> = None;
    for (i, x) in projected.iter().enumerate() {
        if deepest_idx.is_none_or(|d| x * def_side > projected[d] * def_side) {
            deepest_idx = Some(i);
        }
    }
    let mut line = 0.0f32;
    for (i, x) in projected.iter().enumerate() {
        if Some(i) == deepest_idx {
            continue;
        }
        if x * def_side > line * def_side {
            line = *x;
        }
    }
    if ball_x * def_side > line * def_side {
        line = ball_x;
    }
    if line * def_side < 0.0 {
        line = 0.01 * -def_side;
    }
    line
}

// ---------------------------------------------------------------------------
// Movement system (replaces the placeholder player_ai_system)
// ---------------------------------------------------------------------------

/// What deciding where to run needs: who he is, what he can do, what he is
/// inclined to do, what the match has done to him — and the velocity that is
/// the decision itself.
type DecidingPlayer = (
    Entity,
    &'static Position,
    &'static Player,
    &'static Attributes,
    &'static Mentality,
    &'static PlayerMatchState,
    &'static mut Velocity,
);

pub fn select_player_movement(
    time: Res<Time>,
    match_state: Res<MatchState>,
    designation: Res<PossessionDesignation>,
    tactics: Res<TeamTactics>,
    tuning: Res<MatchTuning>,
    ball_query: Query<&Ball, Without<Player>>,
    mut player_query: Query<DecidingPlayer, Without<Ball>>,
) {
    // If a set piece is active (game paused for a restart), freeze everyone.
    if match_state.set_piece != SetPiece::None {
        for (.., mut velocity) in player_query.iter_mut() {
            velocity.0 = Vec3::ZERO;
        }
        return;
    }

    let Ok(ball) = ball_query.single() else {
        return;
    };
    let now_ms = crate::match_clock::engine_elapsed_ms(&time);

    let snaps: Vec<PlayerReading> = player_query
        .iter()
        .map(|(_, position, p, _, _, _, v)| PlayerReading {
            id: p.id,
            playing_position: p.position,
            role: p.role,
            pos: position.on_pitch(),
            vel: Vec2::new(v.0.x, v.0.y),
            formation_slot: p.formation_slot,
        })
        .collect();

    let match_designated = match_state.possession_player.or_else(|| {
        if designation.time_to_ball_ms[TeamId::Home] <= designation.time_to_ball_ms[TeamId::Away] {
            designation.designated[TeamId::Home]
        } else {
            designation.designated[TeamId::Away]
        }
    });

    let ctx = DecisionContext {
        snaps: &snaps,
        sides: match_state.sides,
        ball,
        tactics: &tactics,
        tuning: &tuning,
        designation: &designation,
        match_designated,
        now_ms,
    };

    for (_, position, player, stats, mentality, player_state, mut velocity) in
        player_query.iter_mut()
    {
        let me = PlayerReading {
            id: player.id,
            playing_position: player.position,
            role: player.role,
            pos: position.on_pitch(),
            vel: Vec2::new(velocity.0.x, velocity.0.y),
            formation_slot: player.formation_slot,
        };
        let man_marking = player_state.marking;
        let avg_velocity = player_state.recent_speed;
        let work_rate = mentality.work_rate;

        let is_possessor = match_state.possession_player == Some(player.id);
        let is_designated = designation.designated[me.team()] == Some(player.id);

        let (dir, velo) = if me.playing_position == PlayingPosition::Goalkeeper {
            goalie_movement(&ctx, &me)
        } else if is_possessor {
            carry_movement(&ctx, &me, stats)
        } else if is_designated && ball_winnable(&ctx, &me, match_state.possession_player) {
            // the magnet branch of _MovementCommand: go win the ball
            to_ball_movement(&me, stats, ball)
        } else {
            // A designated player who cannot win the ball plays off it like
            // anyone else: a permanent containment shadow presses the back line
            // nonstop and the presser intercepts the escape passes he forced.
            off_ball_movement(&ctx, &me, man_marking, avg_velocity, work_rate)
        };

        velocity.0 = Vec3::new(dir.x, dir.y, 0.0) * velo;
    }
}

/// The possessor carries the ball: close the gap to the ball, then move along
/// the dribble force field (the knock-ons in the kick system roll it the same
/// way). Approximates `AI_GetBallControlMovement`.
fn carry_movement(ctx: &DecisionContext, me: &PlayerReading, stats: &Attributes) -> (Vec2, f32) {
    let ball_pos = Vec2::new(ctx.ball.predictions[0].x, ctx.ball.predictions[0].y);
    let dist = me.pos.distance(ball_pos);
    if dist > 0.5 {
        (
            (ball_pos - me.pos).normalize_or_zero(),
            stats.top_speed * 0.95,
        )
    } else {
        let all: Vec<(TeamId, Vec2, Vec2)> =
            ctx.snaps.iter().map(|s| (s.team(), s.pos, s.vel)).collect();
        let dir =
            crate::player_movement::dribble_direction(me.pos, me.vel, me.team(), ctx.sides, &all);
        // dribble slower in traffic, open up when free
        let opp_close = ctx
            .snaps
            .iter()
            .any(|s| s.team() != me.team() && s.pos.distance(me.pos) < 6.0);
        let velo = if opp_close {
            WALK_VELOCITY
        } else {
            stats.top_speed * 0.95
        };
        (dir, velo)
    }
}

/// Port of the magnet decision in `_MovementCommand`: the designated player
/// goes to the ball ONLY when it is genuinely winnable — `possessionAmount >
/// 0.99` (we would beat everyone to it), or a loose ball with decent odds
/// (`!oppTeamHasPossession && possessionAmount > 0.5`). Otherwise he behaves
/// like any off-the-ball player (the original's defensive designated branch
/// has autoBias ≈ 0 for AI players).
fn ball_winnable(
    ctx: &DecisionContext,
    me: &PlayerReading,
    possession_player: Option<PlayerId>,
) -> bool {
    let tuning = &ctx.tuning.possession;
    let my_time = ctx.designation.time_to_ball_ms[me.team()].min(tuning.time_to_ball_cap_ms);
    let opp_time =
        ctx.designation.time_to_ball_ms[me.team().opponent()].min(tuning.time_to_ball_cap_ms);
    let softening = tuning.time_to_ball_softening_ms;
    let possession_amount = (opp_time + softening) / (my_time + softening);

    let opp_has_ball = possession_player
        .and_then(|e| snap_of(ctx.snaps, e))
        .is_some_and(|s| s.team() != me.team());

    possession_amount > tuning.winnable_outright
        || (!opp_has_ball && possession_amount > tuning.winnable_loose)
}

/// Run to the earliest reachable point on the ball's predicted path
/// (approximates `AI_GetToBallMovement`).
fn to_ball_movement(me: &PlayerReading, stats: &Attributes, ball: &Ball) -> (Vec2, f32) {
    let (intercept, _) =
        crate::player_movement::find_interception(me.pos, stats.top_speed, &ball.predictions);
    ((intercept - me.pos).normalize_or_zero(), stats.top_speed)
}

/// Off-the-ball movement: hunting/defending (from `RequestCommand`'s movement
/// block) or the per-line default strategy.
fn off_ball_movement(
    ctx: &DecisionContext,
    me: &PlayerReading,
    man_marking: Option<PlayerId>,
    avg_velocity: f32,
    work_rate: f32,
) -> (Vec2, f32) {
    let team = me.team();
    let my_designation_time = ctx.designation.time_to_ball_ms[team];
    let opp_designation_time = ctx.designation.time_to_ball_ms[team.opponent()];
    let team_has_best_possession = my_designation_time <= opp_designation_time;

    // hunt the opponent ball carrier when he's close and we're one of the two
    // closest teammates (port of the "more 'hunting' method" block)
    if !team_has_best_possession
        && man_marking.is_none()
        && let Some(opp) =
            ctx.designation.designated[team.opponent()].and_then(|e| snap_of(ctx.snaps, e))
    {
        let mind_set = me.role.attacking_bias();
        let defending = &ctx.tuning.defending;
        let mut hunt_threshold =
            defending.hunt_distance + (1.0 - mind_set) * defending.hunt_distance_defensive_bonus;
        hunt_threshold *=
            0.5 * 1.0 + 0.5 * (1.0 - normalized_clamp(avg_velocity, 0.0, SPRINT_VELOCITY));
        // match difficulty 1.0 → * (0.3 + 0.7) = 1.0

        let gap = ((opp.pos + opp.vel * 0.12) - (me.pos + me.vel * 0.04)).length();
        if gap < hunt_threshold {
            let hunters = closest_players(ctx.snaps, team, opp.pos + opp.vel * 0.1, None, 2);
            if hunters.iter().any(|s| s.id == me.id) {
                let defend_pos = get_defend_position(me, opp, team, ctx.sides);
                if need_defending_movement(ctx.sides.defending_x(team), me.pos, defend_pos) {
                    let to_target = defend_pos - me.pos;
                    let velo = (to_target.length() * DISTANCE_TO_VELOCITY_MULTIPLIER)
                        .clamp(0.0, SPRINT_VELOCITY);
                    return (to_target.normalize_or_zero(), velo);
                }
            }
        }
    }

    // default strategies (default_def / default_mid / default_off)
    let (attack_bias_min, attack_bias_max, defensive_k, run_gate, use_trap) =
        match me.playing_position {
            PlayingPosition::LeftBack
            | PlayingPosition::CentreBack
            | PlayingPosition::RightBack => (0.2, 0.9, 1.9, f32::MAX, true),
            PlayingPosition::CentreForward => (0.1, 0.6, 1.3, 0.7, false),
            PlayingPosition::Goalkeeper
            | PlayingPosition::DefensiveMidfielder
            | PlayingPosition::CentreMidfielder
            | PlayingPosition::LeftMidfielder
            | PlayingPosition::RightMidfielder
            | PlayingPosition::AttackingMidfielder => (0.1, 0.7, 1.5, 0.9, true),
        };

    let ai = &ctx.tactics.team[team];
    let fading = ai.fading_team_possession_amount;

    let focal_point = ctx
        .match_designated
        .and_then(|e| snap_of(ctx.snaps, e))
        .map(|s| s.pos)
        .unwrap_or(Vec2::new(
            ctx.ball.predictions[0].x,
            ctx.ball.predictions[0].y,
        ));

    let base_position = get_adapted_formation_position(
        ctx.tactics,
        team,
        ctx.sides,
        crate::team_tactics::AdaptedFor {
            player_pos: me.pos,
            formation_pos: me.formation_slot,
            playing_position: me.playing_position,
            role: me.role,
        },
        focal_point,
        ctx.ball,
    );

    // offensive component: blend towards the support position
    let attack_bias = normalized_clamp(fading - 0.5, attack_bias_min, attack_bias_max);
    let make_run = attack_bias > run_gate
        && ai.attacking_run_player == Some(me.id)
        && ai.end_attacking_run_ms > ctx.now_ms;
    let support = get_support_position_force_field(ctx, me, base_position, make_run);
    let mut desired = base_position * (1.0 - attack_bias) + support * attack_bias;

    // defensive component
    let mind_set = me.role.attacking_bias();
    let bias = (defensive_k - mind_set - fading).clamp(0.0, 1.0).powf(0.7);
    add_defensive_component(ctx, me, man_marking, &mut desired, bias);

    if use_trap {
        apply_offside_trap(ctx.tactics, team, ctx.sides, &mut desired);
    }

    let to_target = desired - me.pos;
    let mut velo = to_target.length() * DISTANCE_TO_VELOCITY_MULTIPLIER;
    velo = get_lazy_velocity(ctx, me, velo, avg_velocity, work_rate);
    velo = velo.clamp(0.0, SPRINT_VELOCITY);

    (to_target.normalize_or_zero(), velo)
}

// ---------------------------------------------------------------------------
// GetLazyVelocity (elizacontroller.cpp)
// ---------------------------------------------------------------------------

fn get_lazy_velocity(
    ctx: &DecisionContext,
    me: &PlayerReading,
    desired_velocity: f32,
    avg_velocity: f32,
    work_rate: f32,
) -> f32 {
    let mut adapted = desired_velocity;
    if adapted > SPRINT_VELOCITY {
        adapted = SPRINT_VELOCITY + (adapted - SPRINT_VELOCITY) * 0.1;
    }

    // fatigueFactorInv = 1.0 (stamina modelling comes later)
    let start_laziness_distance = 20.0;
    let end_laziness_distance = 65.0;

    let opp_pos = ctx.designation.designated[me.team().opponent()]
        .and_then(|e| snap_of(ctx.snaps, e))
        .map(|s| s.pos)
        .unwrap_or(Vec2::ZERO);
    let action_distance = (me.pos - opp_pos).length();
    let team_possession =
        (ctx.tactics.team[me.team()].fading_team_possession_amount - 0.5).clamp(0.0, 1.0);
    let mind_set = me.role.attacking_bias();

    let laziness_by_role = mind_set + team_possession * (1.0 - mind_set * 2.0);
    let laziness_by_position = normalized_clamp(
        action_distance,
        start_laziness_distance,
        end_laziness_distance,
    );

    let lazy_factor = laziness_by_position * (0.5 + laziness_by_role * 0.5);
    let mut resulting = adapted * (1.0 - lazy_factor);

    let clamp_to_dribble = desired_velocity >= DRIBBLE_VELOCITY;
    if clamp_to_dribble && resulting < DRIBBLE_VELOCITY {
        resulting = DRIBBLE_VELOCITY;
    }

    // short term fatigue / catching one's breath
    let mut breath = 1.0 - normalized_clamp(avg_velocity, 0.0, SPRINT_VELOCITY);
    breath = breath.powf(0.8 - work_rate * 0.2);
    breath = (breath * 1.2).clamp(0.0, 1.0);
    breath = breath * lazy_factor + (1.0 - lazy_factor);
    resulting.min(SPRINT_VELOCITY * breath)
}

// ---------------------------------------------------------------------------
// Defensive helpers (playercontroller.cpp)
// ---------------------------------------------------------------------------

/// Port of `PlayerController::GetDefendPosition(opp)`: the point on the
/// opp → goal line we can reach as soon as the opponent can.
fn get_defend_position(
    me: &PlayerReading,
    opp: &PlayerReading,
    team: TeamId,
    sides: PitchSides,
) -> Vec2 {
    let side = sides.defending_x(team);
    let goal_pos = Vec2::new(PITCH_HALF_W * side, 0.0);
    let opp_position = opp.pos;
    let opp_to_goal = goal_pos - opp_position;

    let mid_point = (me.pos + opp_position) * 0.5;
    let perp = mid_point + rotated_2d(me.pos - mid_point, std::f32::consts::FRAC_PI_2);

    let (_, u) = line_intersection_2d(opp_position, goal_pos, mid_point, perp);
    let u = u.clamp(0.0, 1.0);
    let mut target = opp_position + (goal_pos - opp_position) * u;

    let opp_to_goal_norm = opp_to_goal.normalize_or_zero();
    target += opp_to_goal_norm * SPRINT_VELOCITY * 0.1 + opp.vel * 0.14;
    target
}

/// Port of `NeedDefendingMovement` (humanoid_utils.cpp): only move when the
/// target is meaningfully wide of our covering position.
fn need_defending_movement(my_side: f32, position: Vec2, target: Vec2) -> bool {
    let mut how_deep = ((target.x - position.x) * -my_side).max(0.0);
    let how_wide = (target.y - position.y).abs();
    how_deep -= 0.5;
    how_wide > how_deep * 0.8
}

/// Port of `PlayerController::AddDefensiveComponent`: pull the desired position
/// towards the goal-covering spot for the man-marked opponent.
fn add_defensive_component(
    ctx: &DecisionContext,
    me: &PlayerReading,
    man_marking: Option<PlayerId>,
    desired_position: &mut Vec2,
    bias: f32,
) {
    let Some(opp) = man_marking.and_then(|e| snap_of(ctx.snaps, e)) else {
        return;
    };
    if me.playing_position == PlayingPosition::Goalkeeper || bias <= 0.0 {
        return;
    }
    let side = ctx.sides.defending_x(me.team());

    let defending = &ctx.tuning.defending;
    let min_distance = defending.cover_min_distance;
    let buffer_distance = defending.cover_buffer_distance;

    let opp_pos = opp.pos + opp.vel * 0.5;
    let shoot_threshold = if ctx.match_designated == Some(opp.id) {
        defending.carrier_threat_distance
    } else {
        defending.generic_threat_distance
    };

    let goal_pos = Vec2::new(PITCH_HALF_W * side, 0.0);
    let opp_to_goal_distance = (goal_pos - opp_pos).length();
    let mut opp_to_threshold_distance =
        (opp_to_goal_distance - shoot_threshold).clamp(min_distance, PITCH_HALF_W);
    let mut shooting_point =
        opp_pos + (goal_pos - opp_pos).normalize_or_zero() * opp_to_threshold_distance;

    // don't cover beyond our own offside trap line
    let trap_x = ctx.tactics.team[me.team()].offside_trap_x;
    if shooting_point.x * side > trap_x * side {
        let (intersect, _) = line_intersection_2d(
            opp_pos,
            goal_pos,
            Vec2::new(trap_x, -PITCH_HALF_H),
            Vec2::new(trap_x, PITCH_HALF_H),
        );
        shooting_point = intersect;
        opp_to_threshold_distance = (shooting_point - opp_pos).length();
    }

    let mut defend_position = *desired_position;
    let me_to_threshold = shooting_point - *desired_position;
    let me_to_threshold_distance = me_to_threshold.length();

    let slacked = me_to_threshold_distance - (opp_to_threshold_distance - buffer_distance);
    if slacked > 0.0 {
        defend_position = *desired_position
            + me_to_threshold.normalize_or_zero() * slacked.clamp(0.0, me_to_threshold_distance);
    }

    // too late: fall back towards our goal instead of chasing the point
    let actual_pos = me.pos + me.vel * 0.14;
    let actual_slacked = (shooting_point - actual_pos).length() - opp_to_threshold_distance;
    if actual_slacked > 0.0 {
        defend_position += (goal_pos - defend_position).normalize_or_zero() * actual_slacked * 0.7;
    }

    *desired_position = *desired_position * (1.0 - bias) + defend_position * bias;
}

// ---------------------------------------------------------------------------
// GetSupportPosition_ForceField (elizacontroller.cpp)
// ---------------------------------------------------------------------------

fn get_support_position_force_field(
    ctx: &DecisionContext,
    me: &PlayerReading,
    base_position: Vec2,
    make_run: bool,
) -> Vec2 {
    let team = me.team();
    let side = ctx.sides.defending_x(team);
    let ai = &ctx.tactics.team[team];

    let designated = ctx.designation.designated[team].and_then(|e| snap_of(ctx.snaps, e));
    let current_pos = me.pos + me.vel * 0.1;
    let main_man_pos = designated
        .map(|d| d.pos + d.vel * 0.1)
        .unwrap_or(current_pos);
    let is_designated = designated.map(|d| d.id) == Some(me.id);

    let dynamic_mind_set = me.role.attacking_bias();

    let base_position_weight = 0.7;
    let overall_weight = 1.0;
    let mut opponent_repel_weight = 0.3 * overall_weight;
    let teammate_repel_weight = 0.4 * overall_weight;
    let ball_repel_weight = 1.0 * overall_weight;
    let run_weight = 1.0 * overall_weight;
    let flock_weight = 0.45 * overall_weight;
    let web_scale = 0.75;

    opponent_repel_weight *= match me.playing_position {
        PlayingPosition::CentreBack | PlayingPosition::LeftBack | PlayingPosition::RightBack => 2.2,
        PlayingPosition::DefensiveMidfielder => 2.0,
        PlayingPosition::CentreMidfielder
        | PlayingPosition::LeftMidfielder
        | PlayingPosition::RightMidfielder => 1.6,
        PlayingPosition::AttackingMidfielder => 1.2,
        PlayingPosition::Goalkeeper | PlayingPosition::CentreForward => 1.0,
    };

    let offside_x = offside_line(ctx.snaps, team, ctx.sides, ctx.ball.predictions[0].x, 240.0);

    let mut force_field: Vec<ForceSpot> = Vec::with_capacity(20);

    // actual base position
    {
        let mut origin = base_position;
        if ai.forward_support_player == Some(me.id) {
            origin.x += -side * (0.3 + 0.7 * dynamic_mind_set) * 12.0;
        } else {
            // lane version: flow forward when in the active lane
            let mut amount = 22.0;
            let lane_y = -main_man_pos.y.signum() * 8.0;
            amount *= curve(
                1.0 - normalized_clamp((lane_y - current_pos.y).abs(), 0.0, 30.0),
                1.0,
            );
            origin.x += -side * dynamic_mind_set.powf(1.5) * amount;
        }
        let mut power = 1.0 * base_position_weight;
        power *= 0.3 + 0.7 * normalized_clamp((origin - current_pos).length(), 0.0, 20.0);
        force_field.push(ForceSpot {
            origin,
            repel: false,
            power,
            falloff: Falloff::Constant,
        });
    }

    if make_run {
        force_field.push(ForceSpot {
            origin: Vec2::new(-side * PITCH_HALF_W, current_pos.y * 0.5),
            repel: false,
            power: 2.0 * run_weight,
            falloff: Falloff::Constant,
        });
    }

    // stay away from opponents (anti-magnet behind them to keep the pass lane open)
    for opp in closest_players(
        ctx.snaps,
        team.opponent(),
        main_man_pos * 0.3 + current_pos * 0.7,
        None,
        3,
    ) {
        let opp_pos = opp.pos + opp.vel * 0.1;
        let origin = opp_pos + (opp_pos - main_man_pos).normalize_or_zero() * 2.0;
        let (radius, power_mul) = if make_run { (2.0, 0.5) } else { (5.0, 1.0) };
        force_field.push(ForceSpot {
            origin,
            repel: true,
            power: opponent_repel_weight * power_mul,
            falloff: Falloff::Curved {
                radius,
                exponent: 0.7,
            },
        });
    }

    // stay away from teammates
    if ai.fading_team_possession_amount >= 1.02 {
        for mate in closest_players(ctx.snaps, team, current_pos, Some(me.id), 6) {
            force_field.push(ForceSpot {
                origin: mate.pos + mate.vel * 0.1,
                repel: true,
                power: teammate_repel_weight,
                falloff: Falloff::Linear {
                    radius: 14.0 * web_scale,
                },
            });
        }
    }

    // stay out of the way of passes / the possession player
    if !is_designated && ai.fading_team_possession_amount >= 1.06 {
        for step in [20usize, 35, 50, 65] {
            let p = ctx.ball.predictions[step.min(ctx.ball.predictions.len() - 1)];
            force_field.push(ForceSpot {
                origin: Vec2::new(p.x, p.y),
                repel: true,
                power: ball_repel_weight,
                falloff: Falloff::Curved {
                    radius: 2.0,
                    exponent: 0.5,
                },
            });
        }
    }

    if !is_designated {
        // attracted to the teammate in possession...
        force_field.push(ForceSpot {
            origin: main_man_pos,
            repel: false,
            power: flock_weight,
            falloff: Falloff::Linear {
                radius: 28.0 * web_scale,
            },
        });
        // ...yet not too close
        force_field.push(ForceSpot {
            origin: main_man_pos,
            repel: true,
            power: flock_weight,
            falloff: Falloff::Linear {
                radius: 16.0 * web_scale,
            },
        });
    }

    let movement = force_field::resolve(&force_field, current_pos, 7.0)
        .map_or(Vec2::ZERO, |direction| direction * SPRINT_VELOCITY);
    let mut position = current_pos + movement;

    // forceNoOffside
    let margin = 0.08;
    if position.x * -side > offside_x * -side - margin {
        position.x = offside_x - margin * -side;
    }

    position.x = position.x.clamp(-PITCH_HALF_W, PITCH_HALF_W);
    position.y = position.y.clamp(-PITCH_HALF_H, PITCH_HALF_H);
    position
}

// ---------------------------------------------------------------------------
// Goalkeeper (goalie_default.cpp)
// ---------------------------------------------------------------------------

fn goalie_movement(ctx: &DecisionContext, me: &PlayerReading) -> (Vec2, f32) {
    let team = me.team();
    let side = ctx.sides.defending_x(team);
    let ball = ctx.ball;

    let keeping = &ctx.tuning.goalkeeping;
    let line_distance = keeping.line_distance;
    let time_to_ball = ctx.designation.time_to_ball_ms[team];
    let pred_ms = keeping.prediction_base_ms + time_to_ball * keeping.prediction_time_share;
    let pred_idx = sample_index(pred_ms, 10.0, ball.predictions.len()).unwrap_or(0);
    let ball_pos = Vec2::new(ball.predictions[pred_idx].x, ball.predictions[pred_idx].y);

    let mut target_pos = Vec2::new((PITCH_HALF_W - line_distance) * side, 0.0);
    let goal_pos = Vec2::new(PITCH_HALF_W * side, 0.0);
    let mut max_velocity = SPRINT_VELOCITY;

    // a loose slow ball inside our box: go fetch it (stand-in for pickup anims)
    let ball_now = Vec2::new(ball.predictions[0].x, ball.predictions[0].y);
    let ball_in_box = ball_now.x * side > PITCH_HALF_W - keeping.box_depth
        && ball_now.y.abs() < keeping.box_half_width;
    let ball_slow = Vec2::new(ball.momentum.x, ball.momentum.y).length()
        < keeping.collectable_speed
        && ball.predictions[0].z < keeping.collectable_height;
    let opp_time = ctx.designation.time_to_ball_ms[team.opponent()];
    if ball_in_box
        && ball_slow
        && ctx
            .match_designated
            .is_none_or(|e| snap_of(ctx.snaps, e).map(|s| s.team()) != Some(team.opponent()))
        && me.pos.distance(ball_now) < keeping.collect_distance
        && opp_time > keeping.collect_opponent_margin_ms
    {
        let dir = (ball_now - me.pos).normalize_or_zero();
        return (dir, SPRINT_VELOCITY);
    }

    if ball_pos.x * side > 0.0 {
        let (bound_for_goal, bound_y) = calculate_ball_bound_for_goal(ball, side, keeping);

        if !bound_for_goal {
            // tactical position: bisect the ball-to-posts angle
            let to_post1 = Vec2::new(PITCH_HALF_W * side, 3.7) - ball_pos;
            let to_post2 = Vec2::new(PITCH_HALF_W * side, -3.7) - ball_pos;
            let angle = to_post2.angle_to(to_post1);
            let middle = normalized_or_2d(rotated_2d(to_post1, angle * 0.5), Vec2::new(side, 0.0));

            let v0 = ball_pos;
            let back_x = (PITCH_HALF_W - 0.7) * side;
            let (mut intersect, _) = line_intersection_2d(
                v0,
                v0 + middle,
                Vec2::new(back_x, -PITCH_HALF_H),
                Vec2::new(back_x, PITCH_HALF_H),
            );
            intersect.y = intersect.y.clamp(-3.7, 3.7);
            let v1 = intersect;

            let mut away_from_goal_offset = 0.7f32;
            let mut away_from_goal_bias = 0.3
                * normalized_clamp(
                    ctx.tactics.team[team].fading_team_possession_amount,
                    1.0,
                    1.5,
                );

            // keeper come-out logic: opponent rushing in with no help nearby
            let mut v0_adapted = v0;
            if ctx.tactics.team[team].fading_team_possession_amount < 1.0
                && let Some(opp) =
                    ctx.designation.designated[team.opponent()].and_then(|e| snap_of(ctx.snaps, e))
            {
                let opp_pos = opp.pos + opp.vel * 0.32;
                let opp_has_ball = ctx.match_designated.is_some_and(|id| id == opp.id);
                v0_adapted = if opp_has_ball {
                    opp_pos * 0.4 + ball_pos * 0.6
                } else {
                    opp_pos * 0.6 + ball_pos * 0.4
                };

                let shoot_threshold = keeping.come_out_threat_distance;
                let opp_to_goal_distance = (goal_pos - opp_pos).length();
                let opp_to_threshold_distance = (opp_to_goal_distance
                    - shoot_threshold
                        * normalized_clamp(opp_to_goal_distance, 0.0, shoot_threshold * 2.0))
                .clamp(0.0, PITCH_HALF_W);
                let shooting_point =
                    opp_pos + (goal_pos - opp_pos).normalize_or_zero() * opp_to_threshold_distance;

                let mate_to_threshold =
                    closest_player(ctx.snaps, team, shooting_point, Some(me.id), false)
                        .and_then(|e| snap_of(ctx.snaps, e))
                        .map(|m| (shooting_point - (m.pos + m.vel * 0.24)).length())
                        .unwrap_or(99999.0);

                if mate_to_threshold > opp_to_threshold_distance + 1.0 {
                    away_from_goal_bias = 1.0;

                    // secondary danger: the opponent's closest helper
                    if let Some(helper) =
                        closest_player(ctx.snaps, team.opponent(), goal_pos, Some(opp.id), false)
                            .and_then(|e| snap_of(ctx.snaps, e))
                    {
                        let helper_pos = helper.pos + helper.vel * 0.32;
                        let helper_shoot_threshold = keeping.helper_threat_distance;
                        let helper_to_goal = (goal_pos - helper_pos).length();
                        let helper_to_threshold = (helper_to_goal
                            - helper_shoot_threshold
                                * normalized_clamp(
                                    helper_to_goal,
                                    0.0,
                                    helper_shoot_threshold * 2.0,
                                ))
                        .clamp(0.0, PITCH_HALF_W);
                        let helper_shooting_point = helper_pos
                            + (goal_pos - helper_pos).normalize_or_zero() * helper_to_threshold;

                        let mate_helper_to_threshold = closest_player(
                            ctx.snaps,
                            team,
                            helper_shooting_point,
                            Some(me.id),
                            false,
                        )
                        .and_then(|e| snap_of(ctx.snaps, e))
                        .map(|m| (helper_shooting_point - (m.pos + m.vel * 0.24)).length())
                        .unwrap_or(99999.0);

                        let mut secondary_distance_diff = 0.0;
                        if mate_helper_to_threshold > helper_to_threshold {
                            secondary_distance_diff = normalized_clamp(
                                mate_helper_to_threshold - helper_to_threshold,
                                0.0,
                                2.0,
                            );
                        }
                        let mut helper_vs_primary = 1.0
                            - normalized_clamp(
                                helper_to_threshold / (opp_to_threshold_distance + 0.0001),
                                1.0,
                                1.5,
                            );
                        helper_vs_primary *= 0.7;
                        away_from_goal_bias =
                            (1.0 - secondary_distance_diff * helper_vs_primary).clamp(0.0, 1.0);
                    }
                }
            }

            let distance = ((v0_adapted - v1).length() - 0.5).max(0.0);
            away_from_goal_offset = cpp_clamp(
                distance * away_from_goal_bias,
                away_from_goal_offset,
                PITCH_HALF_W,
            );

            target_pos = v1 + (v0_adapted - v1).normalize_or_zero() * away_from_goal_offset;

            // going back towards goal along the cover line: slow down
            let (dist_to_line, u) = line_distance_to_point_2d(v0_adapted, v1, me.pos);
            if (target_pos - goal_pos).length() < (me.pos - goal_pos).length()
                && dist_to_line < 1.0
                && u > 0.0
            {
                max_velocity = WALK_VELOCITY;
            }

            target_pos.x = target_pos.x.clamp(-PITCH_HALF_W + 0.2, PITCH_HALF_W - 0.2);
        } else {
            // intercept the incoming shot: cover the crossing point
            max_velocity = SPRINT_VELOCITY;

            let v0 = Vec2::new(ball.predictions[1].x, ball.predictions[1].y);
            let min_goal_line_dist = 0.4;
            let mut over_line = Vec2::new(PITCH_HALF_W * side, bound_y);
            over_line += (v0 - over_line).normalize_or_zero() * min_goal_line_dist;
            let v1 = over_line;

            let (_, u) = line_distance_to_point_2d(v0, v1, me.pos + me.vel * 0.05);
            let u = u.clamp(0.0, 1.0);
            target_pos = v0 + (v1 - v0) * u;
            target_pos.x = target_pos.x.clamp(-PITCH_HALF_W + 0.2, PITCH_HALF_W - 0.2);
        }
    }

    let to_target = target_pos - me.pos;
    let velo = (to_target.length() * DISTANCE_TO_VELOCITY_MULTIPLIER).clamp(0.0, max_velocity);
    (to_target.normalize_or_zero(), velo)
}

/// Port of `GoalieDefaultStrategy::CalculateIfBallIsBoundForGoal` (2D version).
/// Returns (bound_for_goal, y coordinate where it crosses the goal line).
fn calculate_ball_bound_for_goal(
    ball: &Ball,
    side: f32,
    keeping: &GoalkeepingTuning,
) -> (bool, f32) {
    // panic factor with average keeper stats (defensivepositioning/vision 0.5)
    let panic = keeping.panic_factor + (1.0 - 0.5) * 0.5;

    let last = ball.predictions[ball.predictions.len() - 1];
    let p250 = ball.predictions[25.min(ball.predictions.len() - 1)];
    // note: original checks keeper distance to the 250 ms prediction; we check
    // from the goal line instead since this runs for the keeper only
    if last.x * side > PITCH_HALF_W
        && (Vec2::new(PITCH_HALF_W * side, 0.0) - Vec2::new(p250.x, p250.y)).length()
            < keeping.bound_for_goal_range
    {
        let v0 = Vec2::new(ball.predictions[0].x, ball.predictions[0].y);
        let p800 = ball.predictions[80.min(ball.predictions.len() - 1)];
        let v1 = Vec2::new(p800.x, p800.y);
        let (intersect, _) = line_intersection_2d(
            v0,
            v1,
            Vec2::new(PITCH_HALF_W * side, -PITCH_HALF_H),
            Vec2::new(PITCH_HALF_W * side, PITCH_HALF_H),
        );
        if intersect.y.abs() <= 3.7 * panic {
            return (true, intersect.y);
        }
    }
    (false, 0.0)
}

// ---------------------------------------------------------------------------
// On-the-ball decision (GetOnTheBallCommands)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PassKind {
    Short,
    Long,
    High,
}

#[derive(Debug, Clone, Copy)]
pub enum OnBallAction {
    /// Shoot at the goal, aiming at this y on the goal line.
    Shot {
        target_y: f32,
    },
    Pass {
        target: PlayerId,
        aim: Vec2,
        kind: PassKind,
    },
    /// Blast it away from danger (port of `_AddPanicPass`).
    PanicClear,
    Dribble,
}

/// Port of `ElizaController::GetOnTheBallCommands` decision logic, evaluated in
/// the original's command-queue priority: panic → pass → shot → dribble.
#[allow(clippy::too_many_arguments)]
pub fn decide_on_ball_action(
    snaps: &[PlayerReading],
    me_id: PlayerId,
    ball: &Ball,
    designation: &PossessionDesignation,
    held_for: Duration,
    offside_line_x: f32,
    sides: PitchSides,
    tuning: &MatchTuning,
    rng: &mut MatchRng,
) -> OnBallAction {
    let Some(me) = snap_of(snaps, me_id) else {
        return OnBallAction::Dribble;
    };
    let team = me.team();
    let side = sides.defending_x(team);
    let mind_set = me.role.attacking_bias();
    let passing = &tuning.passing;

    let opponents: Vec<&PlayerReading> = snaps.iter().filter(|s| s.team() != team).collect();

    // one-touch difficulty (movement mismatch with the incoming ball)
    let movement_diff = normalized_clamp(
        (Vec2::new(ball.momentum.x, ball.momentum.y) - me.vel).length(),
        0.0,
        10.0,
    );
    let technical_shortpass = 0.5;
    let one_touch_is_hard = movement_diff - technical_shortpass * movement_diff * 0.8;

    let long_possession_factor =
        normalized_clamp(held_for.as_millis() as f32, 0.0, passing.long_possession_ms).powi(2);

    // first selection weights
    let forward_space_weight = 0.4;
    let space_weight = 0.3;
    let forward_weight = 2.0 + mind_set * 6.0;
    let total_weight_1 = forward_space_weight + space_weight + forward_weight;
    let tactical_improvement_threshold = passing.tactical_improvement_threshold * (1.0 - mind_set);

    // second selection weights
    let tactical_diff_weight = 1.0 + mind_set.powi(2) * 10.0;
    let pass_weight = 1.0;
    let pass_minimum = passing.minimum_odds
        + passing.minimum_odds_defensive_bonus * (1.0 - mind_set)
        - long_possession_factor * passing.minimum_odds_long_possession_relief;
    let total_weight_2 = tactical_diff_weight + pass_weight;
    let pass_threshold = passing.combined_threshold
        - long_possession_factor * passing.combined_threshold_long_possession_relief;

    let rate = |s: &PlayerReading| -> f32 {
        let sit = tactical_situation(snaps, s, sides);
        (sit.0 * forward_space_weight + sit.1 * space_weight + sit.2 * forward_weight)
            / total_weight_1
    };

    let my_tactical_rating = rate(me);

    // collect the best pass option
    let mut best_total = 0.0f32;
    let mut best: Option<(PlayerId, Vec2, PassKind, f32)> = None; // + pass rating
    for mate in snaps.iter().filter(|s| s.team() == team && s.id != me_id) {
        // offside receivers are a wasted touch (our addition, see module docs)
        if mate.pos.x * -side > offside_line_x * -side - passing.offside_receiver_margin {
            continue;
        }
        let mut mate_rating = rate(mate);
        if mate.playing_position == PlayingPosition::Goalkeeper {
            // don't like playing back to the goalie
            mate_rating *= passing.keeper_target_penalty;
        }
        if mate_rating <= my_tactical_rating + tactical_improvement_threshold {
            continue;
        }
        let tactical_diff = mate_rating - my_tactical_rating;

        let (odds_short, aim_short) =
            passing_odds_to_player(me, mate, PassKind::Short, &opponents, 1.0, passing, sides);
        let (odds_long, aim_long) =
            passing_odds_to_player(me, mate, PassKind::Long, &opponents, 1.0, passing, sides);
        let (odds_high, aim_high) =
            passing_odds_to_player(me, mate, PassKind::High, &opponents, 1.0, passing, sides);

        let (pass_rating, kind, aim) = if odds_short >= odds_long && odds_short >= odds_high {
            (odds_short, PassKind::Short, aim_short)
        } else if odds_long >= odds_high {
            (odds_long, PassKind::Long, aim_long)
        } else {
            (odds_high, PassKind::High, aim_high)
        };

        let total = (tactical_diff * tactical_diff_weight + pass_rating * pass_weight
            - one_touch_is_hard)
            / total_weight_2;

        if total > best_total && total > pass_threshold && pass_rating > pass_minimum {
            best_total = total;
            best = Some((mate.id, aim, kind, pass_rating));
        }
    }

    // panic (defensive roles close to their own goal under threat)
    let clearance = &tuning.clearance;
    let softening = tuning.possession.time_to_ball_softening_ms;
    let my_time = designation.time_to_ball_ms[team];
    let opp_time = designation.time_to_ball_ms[team.opponent()];
    let possession_amount = (opp_time + softening) / (my_time.max(0.0) + softening);
    if mind_set < clearance.defensive_mindset_max
        && me.playing_position != PlayingPosition::Goalkeeper
    {
        let panic_proneness = 1.0 - mind_set * 2.0;
        let goal_closeness = 1.0
            - normalized_clamp(
                (me.pos - Vec2::new(PITCH_HALF_W * side, 0.0)).length(),
                clearance.goal_closeness_near,
                clearance.goal_closeness_far,
            );
        let best_pass_rating = best.map(|(_, _, _, r)| r).unwrap_or(0.0);
        if best_pass_rating < panic_proneness * goal_closeness
            && possession_amount
                < clearance.possession_threshold
                    + panic_proneness * goal_closeness * clearance.possession_threshold_gain
        {
            return OnBallAction::PanicClear;
        }
    }
    if me.playing_position == PlayingPosition::Goalkeeper
        && possession_amount < tuning.goalkeeping.clearance_possession_threshold
    {
        return OnBallAction::PanicClear;
    }

    if let Some((target, aim, kind, _)) = best {
        return OnBallAction::Pass { target, aim, kind };
    }

    // shoot?
    let shooting = &tuning.shooting;
    let ideal_shot_pos_factor = 1.0
        - normalized_clamp(
            (Vec2::new((PITCH_HALF_W - shooting.ideal_position_offset) * -side, 0.0) - me.pos)
                .length(),
            0.0,
            shooting.ideal_position_range,
        );
    let ideal_shot_pos_factor = curve(ideal_shot_pos_factor, 1.0);
    if ideal_shot_pos_factor > shooting.ideal_position_gate {
        let goal_x = (PITCH_HALF_W + 1.0) * -side;
        let odds_at = |target_y: f32| {
            shot_odds(
                me,
                Vec2::new(goal_x, target_y),
                &opponents,
                shooting.odds_velocity_multiplier,
                passing,
            )
        };
        let mut odds = odds_at(0.0);
        let mut y = 0.0f32;
        let odds1 = odds_at(-shooting.aim_probe_y);
        if odds1 > odds {
            odds = odds1;
            y = -shooting.aim_y;
        }
        let odds3 = odds_at(shooting.aim_probe_y);
        if odds3 > odds {
            odds = odds3;
            y = shooting.aim_y;
        }
        let odds = odds.powf(0.5);
        if odds + rng.range(0.0, shooting.odds_random_span) > shooting.odds_threshold {
            return OnBallAction::Shot { target_y: y };
        }
    }

    // Hemmed in, dribbling on only feeds the herd-out towards the sideline, so
    // any escape pass will do and otherwise it gets blasted clear. The original
    // resolves this with body-shielded turns we do not have.
    let hemmed = opponents
        .iter()
        .filter(|o| (o.pos - me.pos).length() < passing.hemmed_distance)
        .count()
        >= passing.hemmed_opponents;
    if hemmed {
        let mut best_escape: Option<(PlayerId, Vec2, PassKind, f32)> = None;
        for mate in snaps.iter().filter(|s| s.team() == team && s.id != me_id) {
            if mate.pos.x * -side > offside_line_x * -side - passing.offside_receiver_margin {
                continue;
            }
            for kind in [PassKind::Short, PassKind::Long, PassKind::High] {
                let (odds, aim) =
                    passing_odds_to_player(me, mate, kind, &opponents, 1.0, passing, sides);
                if best_escape.is_none_or(|(_, _, _, r)| odds > r) {
                    best_escape = Some((mate.id, aim, kind, odds));
                }
            }
        }
        if let Some((target, aim, kind, odds)) = best_escape
            && odds > passing.escape_minimum_odds
        {
            return OnBallAction::Pass { target, aim, kind };
        }
        return OnBallAction::PanicClear;
    }

    OnBallAction::Dribble
}

/// (forwardSpaceRating, spaceRating, forwardRating) — port of
/// `Player::_CalculateTacticalSituation`.
fn tactical_situation(
    snaps: &[PlayerReading],
    s: &PlayerReading,
    sides: PitchSides,
) -> (f32, f32, f32) {
    let side = sides.defending_x(s.team());
    let opponents: Vec<&PlayerReading> = snaps
        .iter()
        .filter(|o| o.team() != s.team() && o.playing_position != PlayingPosition::Goalkeeper)
        .collect();

    let check_pos = s.pos + Vec2::new(-side, 0.0) * SPRINT_VELOCITY * 0.5;
    let forward_space = free_space(&opponents, check_pos, 5.0, 0.5);

    let check_pos = s.pos + s.vel * 0.1;
    let space = free_space(&opponents, check_pos, 5.0, 0.1);

    let forward = (1.0
        - ((Vec2::new(PITCH_HALF_W * -side, 0.0) - s.pos).length() / (PITCH_HALF_W * 2.0))
            .clamp(0.0, 1.0))
    .powf(1.5);

    (forward_space, space, forward)
}

/// Port of `AI_CalculateFreeSpace`: how free `focus_pos` is from opponents who
/// could close it down within `future_sec`.
fn free_space(
    opponents: &[&PlayerReading],
    focus_pos: Vec2,
    safe_distance: f32,
    future_sec: f32,
) -> f32 {
    let mut current = 0.0f32;
    for opp in opponents {
        let mut pos = opp.pos + opp.vel * 0.2; // slowness
        let mut to_focus =
            (focus_pos - pos).normalize_or_zero() * SPRINT_VELOCITY * (future_sec - 0.2).max(0.0);
        if to_focus.length() > (focus_pos - pos).length() {
            to_focus = focus_pos - pos;
        }
        pos += to_focus;
        current += 1.0 - (pos - focus_pos).length().clamp(0.0, safe_distance) / safe_distance;
    }
    1.0 - normalized_clamp(current, 0.0, 2.5)
}

/// Port of the player-target `_GetPassingOdds` overload; returns (odds, aim point).
fn passing_odds_to_player(
    me: &PlayerReading,
    mate: &PlayerReading,
    kind: PassKind,
    opponents: &[&PlayerReading],
    ball_velocity_multiplier: f32,
    passing: &PassingTuning,
    sides: PitchSides,
) -> (f32, Vec2) {
    let side = sides.defending_x(me.team());
    let initial_distance = (mate.pos - me.pos).length();
    if kind == PassKind::High && initial_distance < passing.high_pass_min_distance {
        return (0.0, mate.pos);
    }
    let estimated_time_sec = 0.7 + initial_distance * 0.03;
    let mut target = mate.pos + mate.vel * estimated_time_sec.clamp(0.0, 0.5);
    if kind == PassKind::Long {
        target += Vec2::new(-side * initial_distance * 0.2, 0.0);
    }
    (
        passing_odds_to_target(
            me,
            target,
            kind,
            opponents,
            ball_velocity_multiplier,
            passing,
        ),
        target,
    )
}

/// Port of the position-target `_GetPassingOdds` overload (elizacontroller.cpp):
/// line-of-pass danger accumulation over opponents.
fn passing_odds_to_target(
    me: &PlayerReading,
    target: Vec2,
    kind: PassKind,
    opponents: &[&PlayerReading],
    ball_velocity_multiplier: f32,
    passing: &PassingTuning,
) -> f32 {
    let second_scale = 1.0;
    let origin = me.pos + me.vel * 0.12;

    let mut danger = 0.0f32;
    for opp in opponents {
        let opp_pos = opp.pos + opp.vel * 0.2;
        let (opp_distance, u) = line_distance_to_point_2d(origin, target, opp_pos);
        if (0.0..=1.2).contains(&u) {
            let applies = match kind {
                PassKind::High => !(0.2..=0.65).contains(&u),
                PassKind::Short | PassKind::Long => true,
            };
            if applies {
                let cu = u.clamp(0.0, 1.0);
                let intersect = origin * (1.0 - cu) + target * cu;
                let opp_to_intersect_sec = (opp_distance + 1.0) / SPRINT_VELOCITY;
                let penalty_time = if kind == PassKind::High && u > 0.5 {
                    2.5
                } else {
                    0.0
                };
                let mut ball_to_intersect_sec =
                    0.7 + (intersect - origin).length() * u * 0.03 + penalty_time;
                ball_to_intersect_sec *= 1.0 / ball_velocity_multiplier;
                danger += (ball_to_intersect_sec - opp_to_intersect_sec + second_scale * 0.5)
                    .clamp(0.0, second_scale);
            }
        }
    }
    if kind == PassKind::High {
        danger += passing.high_pass_danger;
    }
    1.0 - normalized_clamp(danger, 0.0, second_scale)
}

/// Shot odds at a goal-mouth point (the original reuses `_GetPassingOdds` with
/// `e_FunctionType_Shot` and a faster ball).
fn shot_odds(
    me: &PlayerReading,
    target: Vec2,
    opponents: &[&PlayerReading],
    ball_velocity_multiplier: f32,
    passing: &PassingTuning,
) -> f32 {
    passing_odds_to_target(
        me,
        target,
        PassKind::Short,
        opponents,
        ball_velocity_multiplier,
        passing,
    )
}
