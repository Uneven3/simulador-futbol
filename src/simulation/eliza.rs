//! Port of `ElizaController` (onthepitch/player/controller/elizacontroller.cpp)
//! and the relevant parts of its base `PlayerController`, plus the off-the-ball
//! strategies (strategies/offtheball/default_def|mid|off.cpp) and the keeper
//! (goalie_default.cpp).
//!
//! Architecture note: the original controller emits `PlayerCommand`s consumed by
//! the humanoid animation system. This port has no animation layer, so movement
//! commands become a per-tick `Velocity` (in `eliza_movement_system`) and
//! on-the-ball commands become an `OnBallAction` that `player_kick_system`
//! executes as a discrete ball touch.
//!
//! Deliberate simplifications:
//! - No `MentalImage` reaction-time delay: controllers see the true state.
//! - `GetOnTheBallCommands` decides among panic / pass / shot / dribble in the
//!   original queue order; `AI_GetPass` execution (direction/power solving) is
//!   approximated with the existing tuned kick recipes.
//! - Pass receivers beyond the offside line are skipped (our earlier fix; the
//!   original relies on support positioning being offside-aware, which
//!   `get_support_position_force_field` also enforces here).

use bevy::prelude::*;

use crate::data::{
    Ball, MatchRng, MatchState, Player, PlayerRole, PlayerStats, Position, PossessionDesignation,
    SetPiece, Velocity,
};
use crate::math::{
    curve, line_distance_to_point_2d, line_intersection_2d, normalized_clamp, normalized_or_2d,
    rotated_2d,
};
use crate::simulation::team_ai::{
    DISTANCE_TO_VELOCITY_MULTIPLIER, DRIBBLE_VELOCITY, PITCH_HALF_H, PITCH_HALF_W, PlayerSnap,
    SPRINT_VELOCITY, TeamAis, WALK_VELOCITY, apply_offside_trap, closest_player, closest_players,
    cpp_clamp, get_adapted_formation_position, team_side,
};

// ---------------------------------------------------------------------------
// Force field (port of ForceSpot + AI_GetForceFieldMovement)
// ---------------------------------------------------------------------------

pub struct ForceSpot {
    pub origin: Vec2,
    pub repel: bool,
    /// true = constant decay (intensity 1 everywhere), false = variable decay
    pub constant: bool,
    pub power: f32,
    pub scale: f32,
    pub exp: f32,
}

/// Port of `AI_GetForceFieldMovement`: returns a movement vector scaled to
/// sprint velocity.
pub fn force_field_movement(
    force_field: &[ForceSpot],
    current_pos: Vec2,
    attractor_damping_distance: f32,
) -> Vec2 {
    let mut cumul_vec = Vec2::ZERO;
    let mut cumul_force = 0.0;
    for spot in force_field {
        let distance = (spot.origin - current_pos).length();
        let intensity = if spot.constant {
            1.0
        } else {
            let i = (1.0 - distance / spot.scale).clamp(0.0, 1.0);
            if spot.exp != 1.0 { i.powf(spot.exp) } else { i }
        };
        if intensity > 0.0 {
            let mut relative_origin = (spot.origin - current_pos).normalize_or_zero();
            if spot.repel {
                relative_origin = -relative_origin;
            } else if distance < attractor_damping_distance {
                relative_origin *= distance / attractor_damping_distance;
            }
            let force = spot.power * intensity;
            cumul_vec += relative_origin * force;
            cumul_force += force;
        }
    }
    if cumul_force == 0.0 {
        Vec2::ZERO
    } else {
        (cumul_vec / cumul_force) * SPRINT_VELOCITY
    }
}

// ---------------------------------------------------------------------------
// Shared context assembled once per tick
// ---------------------------------------------------------------------------

struct ElizaCtx<'a> {
    snaps: &'a [PlayerSnap],
    ball: &'a Ball,
    team_ais: &'a TeamAis,
    designation: &'a PossessionDesignation,
    /// The single player (either team) expected to reach the ball first
    /// (original `Match::GetDesignatedPossessionPlayer`).
    match_designated: Option<Entity>,
    now_ms: u64,
}

fn snap_of(snaps: &[PlayerSnap], entity: Entity) -> Option<&PlayerSnap> {
    snaps.iter().find(|s| s.entity == entity)
}

/// Offside line faced by attackers of `att_team`: one-but-deepest opponent
/// (projected `future_ms` ahead) or the ball, never inside the attackers' own
/// half (port of `AI_GetOffsideLine`).
pub fn offside_line(snaps: &[PlayerSnap], att_team: u32, ball_x: f32, future_ms: f32) -> f32 {
    let def_team = 1 - att_team;
    let def_side = team_side(def_team);
    let projected: Vec<f32> = snaps
        .iter()
        .filter(|s| s.team == def_team)
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

pub fn eliza_movement_system(
    time: Res<Time>,
    match_state: Res<MatchState>,
    designation: Res<PossessionDesignation>,
    team_ais: Res<TeamAis>,
    ball_query: Query<&Ball, Without<Player>>,
    mut player_query: Query<
        (Entity, &Position, &Player, &PlayerStats, &mut Velocity),
        Without<Ball>,
    >,
) {
    // If a set piece is active (game paused for a restart), freeze everyone.
    if match_state.set_piece != SetPiece::None {
        for (_, _, _, _, mut velocity) in player_query.iter_mut() {
            velocity.0 = Vec3::ZERO;
        }
        return;
    }

    let Ok(ball) = ball_query.single() else {
        return;
    };
    let now_ms = (time.elapsed_secs_f64() * 1000.0) as u64;

    let snaps: Vec<PlayerSnap> = player_query
        .iter()
        .map(|(entity, position, p, _, v)| PlayerSnap {
            entity,
            team: p.team_index,
            role: p.role,
            pos: position.on_pitch(),
            vel: Vec2::new(v.0.x, v.0.y),
            formation_pos: p.formation_pos,
        })
        .collect();

    // (entity, man_marking, avg_velocity, work_rate)
    let extras: Vec<(Entity, Option<Entity>, f32, f32)> = player_query
        .iter()
        .map(|(e, _, p, s, _)| (e, p.man_marking, p.avg_velocity, s.work_rate))
        .collect();

    let match_designated = match_state.possession_player.or_else(|| {
        if designation.time_to_ball_ms[0] <= designation.time_to_ball_ms[1] {
            designation.designated[0]
        } else {
            designation.designated[1]
        }
    });

    let ctx = ElizaCtx {
        snaps: &snaps,
        ball,
        team_ais: &team_ais,
        designation: &designation,
        match_designated,
        now_ms,
    };

    for (entity, position, player, stats, mut velocity) in player_query.iter_mut() {
        let me = PlayerSnap {
            entity,
            team: player.team_index,
            role: player.role,
            pos: position.on_pitch(),
            vel: Vec2::new(velocity.0.x, velocity.0.y),
            formation_pos: player.formation_pos,
        };
        let extra = extras.iter().find(|(e, _, _, _)| *e == entity).unwrap();
        let man_marking = extra.1;
        let avg_velocity = extra.2;
        let work_rate = extra.3;

        let is_possessor = match_state.possession_player == Some(entity);
        let is_designated = designation.designated[me.team as usize] == Some(entity);

        let (dir, velo) = if me.role == PlayerRole::GK {
            goalie_movement(&ctx, &me)
        } else if is_possessor {
            carry_movement(&ctx, &me, stats)
        } else if is_designated && ball_winnable(&ctx, &me, match_state.possession_player) {
            // the magnet branch of _MovementCommand: go win the ball
            to_ball_movement(&me, stats, ball)
        } else {
            // NOTE: a designated player whose ball is NOT winnable behaves like
            // any off-the-ball player (strategy + conditional hunting) — in the
            // original the defensive designated has autoBias ≈ 0. A permanent
            // 1-2 m containment shadow presses the opposing back line nonstop,
            // forces escape passes into the presser, and the presser intercepts
            // them (measured: interceptors were the pressing CFs/CBs).
            off_ball_movement(&ctx, &me, man_marking, avg_velocity, work_rate)
        };

        velocity.0 = Vec3::new(dir.x, dir.y, 0.0) * velo;
    }
}

/// The possessor carries the ball: close the gap to the ball, then move along
/// the dribble force field (the knock-ons in the kick system roll it the same
/// way). Approximates `AI_GetBallControlMovement`.
fn carry_movement(ctx: &ElizaCtx, me: &PlayerSnap, stats: &PlayerStats) -> (Vec2, f32) {
    let ball_pos = Vec2::new(ctx.ball.predictions[0].x, ctx.ball.predictions[0].y);
    let dist = me.pos.distance(ball_pos);
    if dist > 0.5 {
        ((ball_pos - me.pos).normalize_or_zero(), stats.speed * 0.95)
    } else {
        let all: Vec<(u32, Vec2, Vec2)> =
            ctx.snaps.iter().map(|s| (s.team, s.pos, s.vel)).collect();
        let dir =
            crate::simulation::player_movement::dribble_direction(me.pos, me.vel, me.team, &all);
        // dribble slower in traffic, open up when free
        let opp_close = ctx
            .snaps
            .iter()
            .any(|s| s.team != me.team && s.pos.distance(me.pos) < 6.0);
        let velo = if opp_close {
            WALK_VELOCITY
        } else {
            stats.speed * 0.95
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
fn ball_winnable(ctx: &ElizaCtx, me: &PlayerSnap, possession_player: Option<Entity>) -> bool {
    let my_time = ctx.designation.time_to_ball_ms[me.team as usize].min(60_000.0);
    let opp_time = ctx.designation.time_to_ball_ms[(1 - me.team) as usize].min(60_000.0);
    let possession_amount = (opp_time + 200.0) / (my_time + 200.0);

    let opp_has_ball = possession_player
        .and_then(|e| snap_of(ctx.snaps, e))
        .is_some_and(|s| s.team != me.team);

    possession_amount > 0.99 || (!opp_has_ball && possession_amount > 0.5)
}

/// Run to the earliest reachable point on the ball's predicted path
/// (approximates `AI_GetToBallMovement`).
fn to_ball_movement(me: &PlayerSnap, stats: &PlayerStats, ball: &Ball) -> (Vec2, f32) {
    let (intercept, _) = crate::simulation::player_movement::find_interception(
        me.pos,
        stats.speed,
        &ball.predictions,
    );
    ((intercept - me.pos).normalize_or_zero(), stats.speed)
}

/// Off-the-ball movement: hunting/defending (from `RequestCommand`'s movement
/// block) or the per-line default strategy.
fn off_ball_movement(
    ctx: &ElizaCtx,
    me: &PlayerSnap,
    man_marking: Option<Entity>,
    avg_velocity: f32,
    work_rate: f32,
) -> (Vec2, f32) {
    let team = me.team;
    let my_designation_time = ctx.designation.time_to_ball_ms[team as usize];
    let opp_designation_time = ctx.designation.time_to_ball_ms[(1 - team) as usize];
    let team_has_best_possession = my_designation_time <= opp_designation_time;

    // hunt the opponent ball carrier when he's close and we're one of the two
    // closest teammates (port of the "more 'hunting' method" block)
    if !team_has_best_possession && man_marking.is_none() {
        if let Some(opp) =
            ctx.designation.designated[(1 - team) as usize].and_then(|e| snap_of(ctx.snaps, e))
        {
            let mind_set = me.role.mind_set();
            let mut hunt_threshold = 10.0 + (1.0 - mind_set) * 10.0;
            hunt_threshold *=
                0.5 * 1.0 + 0.5 * (1.0 - normalized_clamp(avg_velocity, 0.0, SPRINT_VELOCITY));
            // match difficulty 1.0 → * (0.3 + 0.7) = 1.0

            let gap = ((opp.pos + opp.vel * 0.12) - (me.pos + me.vel * 0.04)).length();
            if gap < hunt_threshold {
                let hunters = closest_players(ctx.snaps, team, opp.pos + opp.vel * 0.1, None, 2);
                if hunters.iter().any(|s| s.entity == me.entity) {
                    let defend_pos = get_defend_position(me, opp, team);
                    if need_defending_movement(team_side(team), me.pos, defend_pos) {
                        let to_target = defend_pos - me.pos;
                        let velo = (to_target.length() * DISTANCE_TO_VELOCITY_MULTIPLIER)
                            .clamp(0.0, SPRINT_VELOCITY);
                        return (to_target.normalize_or_zero(), velo);
                    }
                }
            }
        }
    }

    // default strategies (default_def / default_mid / default_off)
    let (attack_bias_min, attack_bias_max, defensive_k, run_gate, use_trap) = match me.role {
        PlayerRole::LB | PlayerRole::CB | PlayerRole::RB => (0.2, 0.9, 1.9, f32::MAX, true),
        PlayerRole::CF => (0.1, 0.6, 1.3, 0.7, false),
        _ => (0.1, 0.7, 1.5, 0.9, true),
    };

    let ai = &ctx.team_ais.team[team as usize];
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
        ctx.team_ais,
        team,
        me.pos,
        me.formation_pos,
        me.role,
        focal_point,
        ctx.ball,
    );

    // offensive component: blend towards the support position
    let attack_bias = normalized_clamp(fading - 0.5, attack_bias_min, attack_bias_max);
    let make_run = attack_bias > run_gate
        && ai.attacking_run_player == Some(me.entity)
        && ai.end_attacking_run_ms > ctx.now_ms;
    let support = get_support_position_force_field(ctx, me, base_position, make_run);
    let mut desired = base_position * (1.0 - attack_bias) + support * attack_bias;

    // defensive component
    let mind_set = me.role.mind_set();
    let bias = (defensive_k - mind_set - fading).clamp(0.0, 1.0).powf(0.7);
    add_defensive_component(ctx, me, man_marking, &mut desired, bias);

    if use_trap {
        apply_offside_trap(ctx.team_ais, team, &mut desired);
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
    ctx: &ElizaCtx,
    me: &PlayerSnap,
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

    let opp_pos = ctx.designation.designated[(1 - me.team) as usize]
        .and_then(|e| snap_of(ctx.snaps, e))
        .map(|s| s.pos)
        .unwrap_or(Vec2::ZERO);
    let action_distance = (me.pos - opp_pos).length();
    let team_possession =
        (ctx.team_ais.team[me.team as usize].fading_team_possession_amount - 0.5).clamp(0.0, 1.0);
    let mind_set = me.role.mind_set();

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
fn get_defend_position(me: &PlayerSnap, opp: &PlayerSnap, team: u32) -> Vec2 {
    let side = team_side(team);
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
    ctx: &ElizaCtx,
    me: &PlayerSnap,
    man_marking: Option<Entity>,
    desired_position: &mut Vec2,
    bias: f32,
) {
    let Some(opp) = man_marking.and_then(|e| snap_of(ctx.snaps, e)) else {
        return;
    };
    if me.role == PlayerRole::GK || bias <= 0.0 {
        return;
    }
    let side = team_side(me.team);

    let possession_player_shoot_threshold = 24.0;
    let generic_shoot_threshold = 8.0;
    let min_distance = 0.4;
    let buffer_distance = 4.0;

    let opp_pos = opp.pos + opp.vel * 0.5;
    let shoot_threshold = if ctx.match_designated == Some(opp.entity) {
        possession_player_shoot_threshold
    } else {
        generic_shoot_threshold
    };

    let goal_pos = Vec2::new(PITCH_HALF_W * side, 0.0);
    let opp_to_goal_distance = (goal_pos - opp_pos).length();
    let mut opp_to_threshold_distance =
        (opp_to_goal_distance - shoot_threshold).clamp(min_distance, PITCH_HALF_W);
    let mut shooting_point =
        opp_pos + (goal_pos - opp_pos).normalize_or_zero() * opp_to_threshold_distance;

    // don't cover beyond our own offside trap line
    let trap_x = ctx.team_ais.team[me.team as usize].offside_trap_x;
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
    ctx: &ElizaCtx,
    me: &PlayerSnap,
    base_position: Vec2,
    make_run: bool,
) -> Vec2 {
    let team = me.team;
    let side = team_side(team);
    let ai = &ctx.team_ais.team[team as usize];

    let designated = ctx.designation.designated[team as usize].and_then(|e| snap_of(ctx.snaps, e));
    let current_pos = me.pos + me.vel * 0.1;
    let main_man_pos = designated
        .map(|d| d.pos + d.vel * 0.1)
        .unwrap_or(current_pos);
    let is_designated = designated.map(|d| d.entity) == Some(me.entity);

    let dynamic_mind_set = me.role.mind_set();

    let base_position_weight = 0.7;
    let overall_weight = 1.0;
    let mut opponent_repel_weight = 0.3 * overall_weight;
    let teammate_repel_weight = 0.4 * overall_weight;
    let ball_repel_weight = 1.0 * overall_weight;
    let run_weight = 1.0 * overall_weight;
    let flock_weight = 0.45 * overall_weight;
    let web_scale = 0.75;

    opponent_repel_weight *= match me.role {
        PlayerRole::CB | PlayerRole::LB | PlayerRole::RB => 2.2,
        PlayerRole::DM => 2.0,
        PlayerRole::CM | PlayerRole::LM | PlayerRole::RM => 1.6,
        PlayerRole::AM => 1.2,
        _ => 1.0,
    };

    let offside_x = offside_line(ctx.snaps, team, ctx.ball.predictions[0].x, 240.0);

    let mut force_field: Vec<ForceSpot> = Vec::with_capacity(20);

    // actual base position
    {
        let mut origin = base_position;
        if ai.forward_support_player == Some(me.entity) {
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
            constant: true,
            power,
            scale: 1.0,
            exp: 1.0,
        });
    }

    if make_run {
        force_field.push(ForceSpot {
            origin: Vec2::new(-side * PITCH_HALF_W, current_pos.y * 0.5),
            repel: false,
            constant: true,
            power: 2.0 * run_weight,
            scale: 1.0,
            exp: 1.0,
        });
    }

    // stay away from opponents (anti-magnet behind them to keep the pass lane open)
    for opp in closest_players(
        ctx.snaps,
        1 - team,
        main_man_pos * 0.3 + current_pos * 0.7,
        None,
        3,
    ) {
        let opp_pos = opp.pos + opp.vel * 0.1;
        let origin = opp_pos + (opp_pos - main_man_pos).normalize_or_zero() * 2.0;
        let (scale, power_mul) = if make_run { (2.0, 0.5) } else { (5.0, 1.0) };
        force_field.push(ForceSpot {
            origin,
            repel: true,
            constant: false,
            power: opponent_repel_weight * power_mul,
            scale,
            exp: 0.7,
        });
    }

    // stay away from teammates
    if ai.fading_team_possession_amount >= 1.02 {
        for mate in closest_players(ctx.snaps, team, current_pos, Some(me.entity), 6) {
            force_field.push(ForceSpot {
                origin: mate.pos + mate.vel * 0.1,
                repel: true,
                constant: false,
                power: teammate_repel_weight,
                scale: 14.0 * web_scale,
                exp: 1.0,
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
                constant: false,
                power: ball_repel_weight,
                scale: 2.0,
                exp: 0.5,
            });
        }
    }

    if !is_designated {
        // attracted to the teammate in possession...
        force_field.push(ForceSpot {
            origin: main_man_pos,
            repel: false,
            constant: false,
            power: flock_weight,
            scale: 28.0 * web_scale,
            exp: 1.0,
        });
        // ...yet not too close
        force_field.push(ForceSpot {
            origin: main_man_pos,
            repel: true,
            constant: false,
            power: flock_weight,
            scale: 16.0 * web_scale,
            exp: 1.0,
        });
    }

    let mut position = current_pos + force_field_movement(&force_field, current_pos, 7.0);

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

fn goalie_movement(ctx: &ElizaCtx, me: &PlayerSnap) -> (Vec2, f32) {
    let team = me.team;
    let side = team_side(team);
    let ball = ctx.ball;

    let line_distance = 10.0;
    let time_to_ball = ctx.designation.time_to_ball_ms[team as usize];
    let pred_ms = 600.0 + time_to_ball * 0.2;
    let pred_idx = ((pred_ms / 10.0) as usize).min(ball.predictions.len() - 1);
    let ball_pos = Vec2::new(ball.predictions[pred_idx].x, ball.predictions[pred_idx].y);

    let mut target_pos = Vec2::new((PITCH_HALF_W - line_distance) * side, 0.0);
    let goal_pos = Vec2::new(PITCH_HALF_W * side, 0.0);
    let mut max_velocity = SPRINT_VELOCITY;

    // a loose slow ball inside our box: go fetch it (stand-in for pickup anims)
    let ball_now = Vec2::new(ball.predictions[0].x, ball.predictions[0].y);
    let ball_in_box = ball_now.x * side > PITCH_HALF_W - 16.4 && ball_now.y.abs() < 20.0;
    let ball_slow =
        Vec2::new(ball.momentum.x, ball.momentum.y).length() < 4.0 && ball.predictions[0].z < 1.5;
    let opp_time = ctx.designation.time_to_ball_ms[(1 - team) as usize];
    if ball_in_box
        && ball_slow
        && ctx
            .match_designated
            .is_none_or(|e| snap_of(ctx.snaps, e).map(|s| s.team) != Some(1 - team))
        && me.pos.distance(ball_now) < 8.0
        && opp_time > 400.0
    {
        let dir = (ball_now - me.pos).normalize_or_zero();
        return (dir, SPRINT_VELOCITY);
    }

    if ball_pos.x * side > 0.0 {
        let (bound_for_goal, bound_y) = calculate_ball_bound_for_goal(ball, side);

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
                    ctx.team_ais.team[team as usize].fading_team_possession_amount,
                    1.0,
                    1.5,
                );

            // keeper come-out logic: opponent rushing in with no help nearby
            let mut v0_adapted = v0;
            if ctx.team_ais.team[team as usize].fading_team_possession_amount < 1.0 {
                if let Some(opp) = ctx.designation.designated[(1 - team) as usize]
                    .and_then(|e| snap_of(ctx.snaps, e))
                {
                    let opp_pos = opp.pos + opp.vel * 0.32;
                    let opp_has_ball = ctx.match_designated.is_some_and(|e| e == opp.entity);
                    v0_adapted = if opp_has_ball {
                        opp_pos * 0.4 + ball_pos * 0.6
                    } else {
                        opp_pos * 0.6 + ball_pos * 0.4
                    };

                    let shoot_threshold = 20.0;
                    let opp_to_goal_distance = (goal_pos - opp_pos).length();
                    let opp_to_threshold_distance = (opp_to_goal_distance
                        - shoot_threshold
                            * normalized_clamp(opp_to_goal_distance, 0.0, shoot_threshold * 2.0))
                    .clamp(0.0, PITCH_HALF_W);
                    let shooting_point = opp_pos
                        + (goal_pos - opp_pos).normalize_or_zero() * opp_to_threshold_distance;

                    let mate_to_threshold =
                        closest_player(ctx.snaps, team, shooting_point, Some(me.entity), false)
                            .and_then(|e| snap_of(ctx.snaps, e))
                            .map(|m| (shooting_point - (m.pos + m.vel * 0.24)).length())
                            .unwrap_or(99999.0);

                    if mate_to_threshold > opp_to_threshold_distance + 1.0 {
                        away_from_goal_bias = 1.0;

                        // secondary danger: the opponent's closest helper
                        if let Some(helper) =
                            closest_player(ctx.snaps, 1 - team, goal_pos, Some(opp.entity), false)
                                .and_then(|e| snap_of(ctx.snaps, e))
                        {
                            let helper_pos = helper.pos + helper.vel * 0.32;
                            let helper_shoot_threshold = 24.0;
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
                                Some(me.entity),
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
fn calculate_ball_bound_for_goal(ball: &Ball, side: f32) -> (bool, f32) {
    // panic factor with average keeper stats (defensivepositioning/vision 0.5)
    let panic = 1.02 + (1.0 - 0.5) * 0.5;

    let last = ball.predictions[ball.predictions.len() - 1];
    let p250 = ball.predictions[25.min(ball.predictions.len() - 1)];
    // note: original checks keeper distance to the 250 ms prediction; we check
    // from the goal line instead since this runs for the keeper only
    if last.x * side > PITCH_HALF_W
        && (Vec2::new(PITCH_HALF_W * side, 0.0) - Vec2::new(p250.x, p250.y)).length() < 32.0
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
        target: Entity,
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
    snaps: &[PlayerSnap],
    me_entity: Entity,
    ball: &Ball,
    designation: &PossessionDesignation,
    possession_duration_ms: u64,
    offside_line_x: f32,
    rng: &mut MatchRng,
) -> OnBallAction {
    let Some(me) = snap_of(snaps, me_entity) else {
        return OnBallAction::Dribble;
    };
    let team = me.team;
    let side = team_side(team);
    let mind_set = me.role.mind_set();

    let opponents: Vec<&PlayerSnap> = snaps.iter().filter(|s| s.team != team).collect();

    // one-touch difficulty (movement mismatch with the incoming ball)
    let movement_diff = normalized_clamp(
        (Vec2::new(ball.momentum.x, ball.momentum.y) - me.vel).length(),
        0.0,
        10.0,
    );
    let technical_shortpass = 0.5;
    let one_touch_is_hard = movement_diff - technical_shortpass * movement_diff * 0.8;

    let long_possession_factor =
        normalized_clamp(possession_duration_ms as f32, 0.0, 5000.0).powi(2);

    // first selection weights
    let forward_space_weight = 0.4;
    let space_weight = 0.3;
    let forward_weight = 2.0 + mind_set * 6.0;
    let total_weight_1 = forward_space_weight + space_weight + forward_weight;
    let tactical_improvement_threshold = 0.06 * (1.0 - mind_set);

    // second selection weights
    let tactical_diff_weight = 1.0 + mind_set.powi(2) * 10.0;
    let pass_weight = 1.0;
    // +0.15 over the original: our pass execution (no AI_GetPass refinement at
    // the touch moment, no receiver trap anims) loses more 50/50 deliveries, so
    // the odds bar to attempt a pass must sit higher
    let pass_minimum = 0.15 + 0.2 * (1.0 - mind_set) - long_possession_factor * 0.1;
    let total_weight_2 = tactical_diff_weight + pass_weight;
    let pass_threshold = 0.1 - long_possession_factor * 0.05;

    let rate = |s: &PlayerSnap| -> f32 {
        let sit = tactical_situation(snaps, s);
        (sit.0 * forward_space_weight + sit.1 * space_weight + sit.2 * forward_weight)
            / total_weight_1
    };

    let my_tactical_rating = rate(me);

    // collect the best pass option
    let mut best_total = 0.0f32;
    let mut best: Option<(Entity, Vec2, PassKind, f32)> = None; // + pass rating
    for mate in snaps
        .iter()
        .filter(|s| s.team == team && s.entity != me_entity)
    {
        // offside receivers are a wasted touch (our addition, see module docs)
        if mate.pos.x * -side > offside_line_x * -side - 0.5 {
            continue;
        }
        let mut mate_rating = rate(mate);
        if mate.role == PlayerRole::GK {
            mate_rating *= 0.7; // don't like playing back to the goalie
        }
        if mate_rating <= my_tactical_rating + tactical_improvement_threshold {
            continue;
        }
        let tactical_diff = mate_rating - my_tactical_rating;

        let (odds_short, aim_short) =
            passing_odds_to_player(me, mate, PassKind::Short, &opponents, 1.0);
        let (odds_long, aim_long) =
            passing_odds_to_player(me, mate, PassKind::Long, &opponents, 1.0);
        let (odds_high, aim_high) =
            passing_odds_to_player(me, mate, PassKind::High, &opponents, 1.0);

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
            best = Some((mate.entity, aim, kind, pass_rating));
        }
    }

    // panic (defensive roles close to their own goal under threat)
    let my_time = designation.time_to_ball_ms[team as usize];
    let opp_time = designation.time_to_ball_ms[(1 - team) as usize];
    let possession_amount = (opp_time + 200.0) / (my_time.max(0.0) + 200.0);
    if mind_set < 0.25 && me.role != PlayerRole::GK {
        let panic_proneness = 1.0 - mind_set * 2.0;
        let goal_closeness = 1.0
            - normalized_clamp(
                (me.pos - Vec2::new(PITCH_HALF_W * side, 0.0)).length(),
                2.0,
                16.0,
            );
        let best_pass_rating = best.map(|(_, _, _, r)| r).unwrap_or(0.0);
        if best_pass_rating < panic_proneness * goal_closeness
            && possession_amount < 0.9 + panic_proneness * goal_closeness * 0.8
        {
            return OnBallAction::PanicClear;
        }
    }
    if me.role == PlayerRole::GK && possession_amount < 3.0 {
        return OnBallAction::PanicClear;
    }

    if let Some((target, aim, kind, _)) = best {
        return OnBallAction::Pass { target, aim, kind };
    }

    // shoot?
    let ideal_shot_pos_factor = 1.0
        - normalized_clamp(
            (Vec2::new((PITCH_HALF_W - 7.0) * -side, 0.0) - me.pos).length(),
            0.0,
            16.0,
        );
    let ideal_shot_pos_factor = curve(ideal_shot_pos_factor, 1.0);
    if ideal_shot_pos_factor > 0.1 {
        let goal_x = (PITCH_HALF_W + 1.0) * -side;
        let mut odds = shot_odds(me, Vec2::new(goal_x, 0.0), &opponents);
        let mut y = 0.0f32;
        let odds1 = shot_odds(me, Vec2::new(goal_x, -3.6), &opponents);
        if odds1 > odds {
            odds = odds1;
            y = -3.5;
        }
        let odds3 = shot_odds(me, Vec2::new(goal_x, 3.6), &opponents);
        if odds3 > odds {
            odds = odds3;
            y = 3.5;
        }
        let odds = odds.powf(0.5);
        if odds + rng.range(0.0, 0.5) > 0.5 {
            return OnBallAction::Shot { target_y: y };
        }
    }

    // hemmed in (2+ opponents on top): dribbling on only feeds the herd-out
    // towards the sideline — look for ANY escape pass (ignoring the tactical
    // improvement threshold), else blast it clear. The original resolves this
    // with body-shielded turn animations we don't have.
    let hemmed = opponents
        .iter()
        .filter(|o| (o.pos - me.pos).length() < 3.0)
        .count()
        >= 2;
    if hemmed {
        let mut best_escape: Option<(Entity, Vec2, PassKind, f32)> = None;
        for mate in snaps
            .iter()
            .filter(|s| s.team == team && s.entity != me_entity)
        {
            if mate.pos.x * -side > offside_line_x * -side - 0.5 {
                continue;
            }
            for kind in [PassKind::Short, PassKind::Long, PassKind::High] {
                let (odds, aim) = passing_odds_to_player(me, mate, kind, &opponents, 1.0);
                if best_escape.is_none_or(|(_, _, _, r)| odds > r) {
                    best_escape = Some((mate.entity, aim, kind, odds));
                }
            }
        }
        if let Some((target, aim, kind, odds)) = best_escape {
            if odds > 0.2 {
                return OnBallAction::Pass { target, aim, kind };
            }
        }
        return OnBallAction::PanicClear;
    }

    OnBallAction::Dribble
}

/// (forwardSpaceRating, spaceRating, forwardRating) — port of
/// `Player::_CalculateTacticalSituation`.
fn tactical_situation(snaps: &[PlayerSnap], s: &PlayerSnap) -> (f32, f32, f32) {
    let side = team_side(s.team);
    let opponents: Vec<&PlayerSnap> = snaps
        .iter()
        .filter(|o| o.team != s.team && o.role != PlayerRole::GK)
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
    opponents: &[&PlayerSnap],
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
    me: &PlayerSnap,
    mate: &PlayerSnap,
    kind: PassKind,
    opponents: &[&PlayerSnap],
    ball_velocity_multiplier: f32,
) -> (f32, Vec2) {
    let side = team_side(me.team);
    let initial_distance = (mate.pos - me.pos).length();
    if kind == PassKind::High && initial_distance < 10.0 {
        return (0.0, mate.pos);
    }
    let estimated_time_sec = 0.7 + initial_distance * 0.03;
    let mut target = mate.pos + mate.vel * estimated_time_sec.clamp(0.0, 0.5);
    if kind == PassKind::Long {
        target += Vec2::new(-side * initial_distance * 0.2, 0.0);
    }
    (
        passing_odds_to_target(me, target, kind, opponents, ball_velocity_multiplier),
        target,
    )
}

/// Port of the position-target `_GetPassingOdds` overload (elizacontroller.cpp):
/// line-of-pass danger accumulation over opponents.
fn passing_odds_to_target(
    me: &PlayerSnap,
    target: Vec2,
    kind: PassKind,
    opponents: &[&PlayerSnap],
    ball_velocity_multiplier: f32,
) -> f32 {
    let second_scale = 1.0;
    let origin = me.pos + me.vel * 0.12;

    let mut danger = 0.0f32;
    for opp in opponents {
        let opp_pos = opp.pos + opp.vel * 0.2;
        let (opp_distance, u) = line_distance_to_point_2d(origin, target, opp_pos);
        if u >= 0.0 && u <= 1.2 {
            let applies = match kind {
                PassKind::High => u < 0.2 || u > 0.65,
                _ => true,
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
        danger += 0.4;
    }
    1.0 - normalized_clamp(danger, 0.0, second_scale)
}

/// Shot odds at a goal-mouth point (the original reuses `_GetPassingOdds` with
/// `e_FunctionType_Shot` and a 3.0 ball velocity multiplier).
fn shot_odds(me: &PlayerSnap, target: Vec2, opponents: &[&PlayerSnap]) -> f32 {
    passing_odds_to_target(me, target, PassKind::Short, opponents, 3.0)
}
