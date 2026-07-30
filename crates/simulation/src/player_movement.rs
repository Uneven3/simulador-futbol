use crate::SimulationSet;
use crate::ball_physics::touch_ball;
use crate::player_decisions::{self, OnBallAction, PassKind};
use crate::team_tactics::{self, PlayerReading, TeamTactics, team_side};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use std::time::Duration;

use crate::diagnostics::{MatchFact, MatchTelemetry, PossessionCause, ReleaseKind};
use football_domain::math::{normalized_clamp, normalized_or_2d, sign_side};
use football_domain::tuning::ContestTuning;
use football_domain::{
    Attributes, Ball, BallTouched, ByTeam, Facing, MatchRng, MatchState, MatchTuning, Player,
    PlayerId, PlayerMatchState, PlayerRegistry, PlayingPosition, Position, PossessionDesignation,
    SetPiece, TeamId, Velocity,
};

/// Everything the ball systems need to read off a body, plus the one thing they
/// write: when he last touched the ball.
pub type BallSystemBody = (
    Entity,
    &'static Position,
    &'static Player,
    &'static Attributes,
    &'static mut PlayerMatchState,
    &'static Velocity,
);

/// Who touched the ball and which body he is right now. The pair travels
/// together because one is the identity and the other is only how to reach it
/// this tick.
#[derive(Clone, Copy)]
struct Toucher {
    player: PlayerId,
    body: Entity,
}

pub struct PlayerMovementPlugin;

impl Plugin for PlayerMovementPlugin {
    fn build(&self, app: &mut App) {
        // `MatchRng` is deliberately not defaulted here: the seed belongs to the
        // scenario, which `MatchSetupPlugin` installs (law 11).
        app.insert_resource(PossessionDesignation::default())
            .init_resource::<TeamTactics>()
            .add_systems(
                FixedUpdate,
                (
                    (
                        update_possession_designation,
                        team_tactics::update_team_tactics,
                        player_decisions::select_player_movement,
                        apply_player_velocity,
                        resolve_player_overlap,
                    )
                        .chain()
                        .in_set(SimulationSet::Players),
                    player_kick_system.in_set(SimulationSet::Kicks),
                ),
            );
    }
}

/// Port of the original team bookkeeping (`Team::GetTimeNeededToGetToBall_ms` +
/// `GetDesignatedTeamPossessionPlayer`): per team, find the single outfield
/// player who can reach the ball's predicted path first. Only he chases; a
/// player in possession is always his team's designated player.
fn update_possession_designation(
    mut match_state: ResMut<MatchState>,
    records: Res<football_domain::OffsideRecords>,
    mut designation: ResMut<PossessionDesignation>,
    registry: Res<PlayerRegistry>,
    ball_query: Query<&Ball>,
    player_query: Query<(&Position, &Player, &Attributes)>,
) {
    let Ok(ball) = ball_query.single() else {
        return;
    };

    let mut best: ByTeam<Option<(PlayerId, f32)>> = ByTeam::default();
    for (position, player, stats) in player_query.iter() {
        if player.position == PlayingPosition::Goalkeeper {
            continue;
        }
        // a player caught in an offside position when the ball was last played
        // must not go for it (or the whistle blows the moment he touches it)
        if records.team == Some(player.id.team)
            && records.players.iter().any(|(id, _)| *id == player.id)
        {
            continue;
        }
        let (_, time_ms) =
            find_interception(position.on_pitch(), stats.top_speed, &ball.predictions);
        let slot = &mut best[player.id.team];
        if slot.is_none_or(|(_, t)| time_ms < t) {
            *slot = Some((player.id, time_ms));
        }
    }

    // whoever holds the ball is his team's designated player by definition
    if let Some(possessor) = match_state.possession_player {
        best[possessor.team] = Some((possessor, 0.0));
    }

    // a pass is only "in flight" while the ball travels; once it truly dies the
    // normal designation race resumes (otherwise a lost pass freezes the team).
    // The threshold must be near-rest: the solved passes ARRIVE dying at the
    // receiver, and expiring at 1.0 m/s stripped the receiver of his priority
    // and trap reach exactly at the moment of reception.
    if ball.momentum.length() < 0.3 {
        match_state.pass_target = None;
    }

    // the intended receiver of a pass in flight attacks it, even if a teammate
    // is nominally a bit faster to the ball (the original's receivers run onto
    // `AI_GetPass` balls; without this the passer often re-chases his own pass)
    if let Some(receiver) = match_state.pass_target
        && let Some(body) = registry.body(receiver)
        && let Ok((position, player, stats)) = player_query.get(body)
    {
        let (_, time_ms) =
            find_interception(position.on_pitch(), stats.top_speed, &ball.predictions);
        let slot = &mut best[player.id.team];
        if time_ms < 3500.0 && slot.is_none_or(|(_, t)| time_ms < t * 1.5 + 300.0) {
            *slot = Some((receiver, time_ms));
        }
    }

    for team in TeamId::BOTH {
        designation.designated[team] = best[team].map(|(id, _)| id);
        designation.time_to_ball_ms[team] = best[team].map_or(f32::MAX, |(_, t)| t);
    }
}

/// Kinematic integration of player velocities (players are not physics bodies,
/// as in the original engine). Also derives the body's facing from the movement
/// and maintains the ~10 s average velocity used by `GetLazyVelocity` (original
/// `Player::GetAverageVelocity(10)`).
fn apply_player_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Position, &mut Facing, &Velocity, &mut PlayerMatchState)>,
) {
    let dt = time.delta_secs();
    for (mut position, mut facing, velocity, mut player_state) in query.iter_mut() {
        position.0.x += velocity.0.x * dt;
        position.0.y += velocity.0.y * dt;
        // Facing follows the movement instantly: turning is not yet a limited
        // capability (MVP 3). A standing player keeps his previous facing.
        if let Ok(direction) = Dir2::new(Vec2::new(velocity.0.x, velocity.0.y)) {
            facing.0 = direction;
        }
        let speed = velocity.0.length();
        player_state.recent_speed += (speed - player_state.recent_speed) * (dt / 10.0).min(1.0);
    }
}

/// Positional separation between player bodies: two bodies of radius 0.35 m
/// cannot occupy the same spot, so overlapping pairs get pushed apart. This is
/// the cheap stand-in for body contact until the motor model of MVP 3; without
/// it, both designated players superimpose on the contested ball.
fn resolve_player_overlap(mut query: Query<&mut Position, With<Player>>) {
    const MIN_DIST: f32 = 0.7; // two body radii

    let mut combinations = query.iter_combinations_mut();
    while let Some([mut a, mut b]) = combinations.fetch_next() {
        let mut delta = b.0 - a.0;
        delta.z = 0.0;
        let dist = delta.length();
        if dist >= MIN_DIST {
            continue;
        }
        // fully overlapping: pick an arbitrary but deterministic axis
        let dir = if dist < 1e-5 { Vec3::X } else { delta / dist };
        let push = (MIN_DIST - dist) * 0.5;
        a.0 -= dir * push;
        b.0 += dir * push;
    }
}

/// Kicking & Dribbling System:
/// Possession acquisition, tackles, dribble knock-ons and pass/shot decisions.
/// Every ball contact is a discrete touch that sets the ball's momentum — the
/// ball is never glued to a player (original interaction model).
// A Bevy system states its dependencies as parameters; the seven-argument rule
// is about call sites, and this one has none.
#[allow(clippy::too_many_arguments)]
fn player_kick_system(
    mut match_state: ResMut<MatchState>,
    designation: Res<PossessionDesignation>,
    offside_records: Res<football_domain::OffsideRecords>,
    pitch: Res<football_domain::PitchConfig>,
    tuning: Res<MatchTuning>,
    mut rng: ResMut<MatchRng>,
    mut ball_query: Query<(&mut Position, &mut Ball), Without<Player>>,
    registry: Res<PlayerRegistry>,
    mut telemetry: ResMut<MatchTelemetry>,
    mut player_query: Query<BallSystemBody, Without<Ball>>,
    time: Res<Time>,
    mut touched_writer: MessageWriter<BallTouched>,
) {
    if match_state.set_piece != SetPiece::None {
        return;
    }

    let Ok((mut ball_position, mut ball)) = ball_query.single_mut() else {
        return;
    };

    let ball_pos = ball_position.0;
    let ball_pos_2d = ball_position.on_pitch();
    let now = time.elapsed();
    let contest = &tuning.contest;

    // 0. Possession is positional: if the ball escaped the possessor, it's loose
    if let Some(possessor) = match_state.possession_player {
        let lost = match registry.body(possessor).map(|body| player_query.get(body)) {
            Some(Ok((_, position, ..))) => {
                position.on_pitch().distance(ball_pos_2d) > contest.possession_escape_distance
            }
            _ => true,
        };
        if lost {
            telemetry.record(MatchFact::PossessionLost {
                player: possessor,
                at: ball_pos_2d,
            });
            match_state.possession_player = None;
        }
    }

    // 1. Possession Tackle Check: closest player in contact range
    let mut closest_player: Option<(PlayerId, Entity)> = None;
    let mut closest_player_dist = f32::MAX;

    for (entity, player_position, player, _, _, _) in player_query.iter() {
        let dist_2d = ball_pos_2d.distance(player_position.on_pitch());

        // players flagged offside at the last touch hold off the ball
        if offside_records.team == Some(player.id.team)
            && offside_records
                .players
                .iter()
                .any(|(id, _)| *id == player.id)
        {
            continue;
        }

        // Standard contact distance to pick up a loose ball is 0.65m (with ball
        // on the ground z < 1.5). The intended pass receiver gets an extended
        // trap reach (the original's trap anims stretch a leg ~1 m via
        // GetBestCheatableAnim) — without it, reception is a coin flip against
        // the marker standing 1-2 m goal-side and almost no pass completes.
        let reach = if match_state.pass_target == Some(player.id) {
            contest.receiver_trap_reach
        } else {
            contest.loose_ball_reach
        };
        if dist_2d < reach && ball_pos.z < contest.max_touch_height {
            // the receiver's stretched reach wins ties inside his radius
            let effective_dist = if match_state.pass_target == Some(player.id) {
                dist_2d - contest.receiver_tie_break
            } else {
                dist_2d
            };
            if effective_dist < closest_player_dist {
                closest_player_dist = effective_dist;
                closest_player = Some((player.id, entity));
            }
        }
    }
    let closest_player_dist = closest_player_dist.max(0.0);

    if let Some((challenger, challenger_body)) = closest_player {
        if let Some(current_possessor) = match_state.possession_player {
            if challenger != current_possessor
                && let Some(Ok((_, current_position, ..))) = registry
                    .body(current_possessor)
                    .map(|b| player_query.get(b))
            {
                // Teammates cannot steal the ball from each other
                if challenger.team != current_possessor.team {
                    // Only the designated possession player makes deliberate
                    // tackles (everyone else holds shape); tight contact
                    // (< 0.50m) and tackle cooldowns still apply
                    let is_designated_tackler =
                        designation.designated[challenger.team] == Some(challenger);
                    // Shielding: in the original the carrier's body (via the
                    // ball-control animations) physically screens the ball, so
                    // a steal needs (a) the tackler genuinely closer to the
                    // ball, and (b) the ball NOT under close control — either
                    // poked loose (> 1 m from the carrier's feet) or held so
                    // long that pressure legitimately forces the turnover.
                    // Without this, the two designated players trade the ball
                    // every cooldown window and the match degenerates into a
                    // stealing metronome.
                    let carrier_dist = current_position.on_pitch().distance(ball_pos_2d);
                    let wins_duel = closest_player_dist < carrier_dist * contest.duel_advantage;
                    let held = now.saturating_sub(match_state.possession_since);
                    let ball_stealable = carrier_dist > contest.shielding_release_distance
                        || held > contest.shielding_release_time;
                    if is_designated_tackler
                        && closest_player_dist < contest.tackle_contact_distance
                        && wins_duel
                        && ball_stealable
                    {
                        // A player who just lost the ball has to wait longer to
                        // win it back, or the two designated players trade it
                        // every cooldown window.
                        let is_prev = match_state.previous_possessor == Some(challenger);
                        let cooldown = if is_prev {
                            contest.regain_cooldown
                        } else {
                            contest.steal_cooldown
                        };

                        if now.saturating_sub(match_state.possession_since) > cooldown {
                            // Tackle successful: the tackler traps the ball
                            match_state.previous_possessor = Some(current_possessor);
                            match_state.possession_player = Some(challenger);
                            match_state.possession_team = Some(challenger.team);
                            match_state.possession_since = now;
                            match_state.pass_target = None;
                            telemetry.record(MatchFact::PossessionGained {
                                player: challenger,
                                from: Some(current_possessor),
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
                                contest,
                                &mut player_query,
                                &mut touched_writer,
                                &mut telemetry,
                            );
                        }
                    }
                }
            }
        } else {
            // Ball is loose: anyone can pick it up, except:
            // 1. Global pickup cooldown after any kick
            let global_cooldown_ok =
                now.saturating_sub(ball.last_touch_at) > contest.loose_ball_cooldown;
            // 2. The last toucher waits longer: the ball has to leave his feet
            let is_last_toucher = ball.last_touch_player == Some(challenger);
            let individual_cooldown_ok = !is_last_toucher
                || (now.saturating_sub(ball.last_touch_at) > contest.own_ball_cooldown);
            // 3. Touch bias (original `GetLastTouchBias`): the player who just
            // played the ball keeps priority in a shoulder-to-shoulder race — an
            // opponent must arrive CLEARLY first to win the loose ball,
            // otherwise the dribbler loses every knock-on to a coin flip.
            let mut touch_bias_ok = true;
            if !is_last_toucher
                && now.saturating_sub(ball.last_touch_at) < contest.touch_bias_window
                && let Some(last_toucher) = ball.last_touch_player
                && let Some(Ok((_, toucher_position, ..))) =
                    registry.body(last_toucher).map(|b| player_query.get(b))
                && last_toucher.team != challenger.team
            {
                let toucher_dist = toucher_position.on_pitch().distance(ball_pos_2d);
                if toucher_dist < contest.touch_bias_distance
                    && closest_player_dist > toucher_dist - contest.touch_bias_margin
                {
                    touch_bias_ok = false;
                }
            }

            if global_cooldown_ok && individual_cooldown_ok && touch_bias_ok {
                telemetry.record(MatchFact::PossessionGained {
                    player: challenger,
                    from: None,
                    cause: PossessionCause::LooseBall,
                    at: ball_pos_2d,
                });
                match_state.previous_possessor = None;
                match_state.possession_player = Some(challenger);
                match_state.possession_team = Some(challenger.team);
                match_state.possession_since = now;
                match_state.pass_target = None;
                control_touch(
                    &mut ball,
                    &mut ball_position,
                    Toucher {
                        player: challenger,
                        body: challenger_body,
                    },
                    now,
                    contest,
                    &mut player_query,
                    &mut touched_writer,
                    &mut telemetry,
                );
            }
        }
    }

    // 2. Play Actions for the player currently in possession
    if let Some(possessor) = match_state.possession_player {
        let Some(possessor_body) = registry.body(possessor) else {
            return;
        };
        let Ok((_, player_position, player, stats, _, velocity)) = player_query.get(possessor_body)
        else {
            return;
        };
        let player_pos_2d = player_position.on_pitch();
        let player_vel_2d = Vec2::new(velocity.0.x, velocity.0.y);
        let player_team = possessor.team;
        let playing_position = player.position;
        let player_speed = stats.top_speed;
        let technical_shot = stats.shot_technique;

        // A touch requires the ball at the feet
        let ball_in_reach = ball_pos_2d.distance(player_pos_2d) < contest.ball_at_feet_distance
            && ball_pos.z < contest.ball_at_feet_height;
        // Goalkeepers clear/kick immediately without dribble delay.
        // Deliberate releases (pass/shot/clear) only need a short reaction time
        // — the original chains trap → pass through its command queue, and a
        // pressed carrier who must idle 350 ms just feeds the stealing loop.
        // Dribble knock-ons keep the slower 350 ms touch cadence.
        let is_gk = playing_position == PlayingPosition::Goalkeeper;
        let since_touch = now.saturating_sub(ball.last_touch_at);
        let can_decide = since_touch > contest.decision_cadence || is_gk;
        let can_knock_on = since_touch > contest.knock_on_cadence || is_gk;

        if !(ball_in_reach && can_decide) {
            return;
        }

        let kick = |ball: &mut Ball,
                    ball_position: &mut Position,
                    momentum: Vec3,
                    spin: Vec3,
                    touched_writer: &mut MessageWriter<BallTouched>| {
            touch_ball(ball, ball_position, momentum);
            ball.set_rotation(spin.x, spin.y, spin.z, 1.0);
            ball.last_touch_team = Some(player_team);
            ball.last_touch_player = Some(possessor);
            ball.last_touch_at = now;
            touched_writer.write(BallTouched { player: possessor });
        };

        // Decision (port of ElizaController::GetOnTheBallCommands), evaluated in
        // the original's command-queue priority: panic → pass → shot → dribble.
        let snaps: Vec<PlayerReading> = player_query
            .iter()
            .map(|(_, position, p, _, _, v)| PlayerReading {
                id: p.id,
                playing_position: p.position,
                role: p.role,
                pos: position.on_pitch(),
                vel: Vec2::new(v.0.x, v.0.y),
                formation_slot: p.formation_slot,
            })
            .collect();
        let side = team_side(player_team);
        let off_line = player_decisions::offside_line(&snaps, player_team, ball_pos.x, 0.0);
        let held_for = now.saturating_sub(match_state.possession_since);
        let action = player_decisions::decide_on_ball_action(
            &snaps,
            possessor,
            &ball,
            &designation,
            held_for,
            off_line,
            &tuning,
            &mut rng,
        );

        let mut release_possession = false;
        match action {
            OnBallAction::Shot { target_y } => {
                // execution keeps the tuned recipe: aim at the goal LINE (±55) —
                // aiming short makes every diagonal drift wide — with curl back
                // inside the far post and a modest topspin dip.
                let shooting = &tuning.shooting;
                let opponent_goal_x = 55.0 * -side;
                let spread = 1.0 - technical_shot;
                let y_aim = target_y * shooting.aim_centre_pull + rng.range(-spread, spread);
                let dir_2d =
                    (Vec2::new(opponent_goal_x, y_aim) - player_pos_2d).normalize_or_zero();
                let goal_dist_factor = normalized_clamp(
                    player_pos_2d.distance(Vec2::new(opponent_goal_x, 0.0)),
                    0.0,
                    shooting.power_distance_range,
                );
                let lift = shooting.lift + goal_dist_factor * shooting.lift_distance_gain;
                let kick_dir = Vec3::new(dir_2d.x, dir_2d.y, lift).normalize_or_zero();
                // original: desiredPower = random(0.7, 1.0) * (0.6 + goalDist * 0.4)
                let (min_power_draw, max_power_draw) = shooting.power_random_range;
                let kick_power = rng.range(min_power_draw, max_power_draw)
                    * (shooting.power_distance_base
                        + goal_dist_factor * shooting.power_distance_gain)
                    * shooting.power_scale
                    + shooting.power_floor;

                let topspin = shooting.topspin;
                let side_spin = -dir_2d.y.signum() * kick_dir.x.signum() * shooting.sidespin;
                let spin = Vec3::new(topspin * kick_dir.y, topspin * kick_dir.x, side_spin);

                kick(
                    &mut ball,
                    &mut ball_position,
                    kick_dir * kick_power,
                    spin,
                    &mut touched_writer,
                );
                match_state.pass_target = None;
                telemetry.record(MatchFact::BallReleased {
                    player: possessor,
                    kind: ReleaseKind::Shot,
                    aim: Vec2::new(opponent_goal_x, y_aim),
                });
                release_possession = true;
            }
            OnBallAction::Pass { target, aim, kind } => {
                // lift per AI_GetAutoPass (0.11 ground / high drops with range);
                // power solved against the real ball physics so the pass
                // ARRIVES at the receiver instead of dying short (short passes
                // dying en route were feeding the interception rate)
                let pass_dist = player_pos_2d.distance(aim);
                // the pass must arrive WITH PACE: a ball that dies at the
                // receiver crawls its last meters and any opponent reading the
                // (perfect) prediction collects that slow tail — measured as
                // ~90% of pass turnovers happening en route. The receiver's
                // extended trap reach + designation priority let him kill the
                // faster ball, like the original's trap anims do.
                let passing = &tuning.passing;
                let (lift, pace_bonus) = match kind {
                    PassKind::Short => (passing.short_lift, passing.short_pace),
                    PassKind::Long => (passing.long_lift, passing.long_pace),
                    PassKind::High => (
                        passing.high_lift
                            - normalized_clamp(pass_dist, 0.0, 60.0)
                                * passing.high_lift_range_relief,
                        passing.high_pace,
                    ),
                };
                let momentum = crate::ball_physics::solve_pass_momentum(
                    &pitch, ball_pos, aim, lift, pace_bonus,
                );
                kick(
                    &mut ball,
                    &mut ball_position,
                    momentum,
                    Vec3::ZERO,
                    &mut touched_writer,
                );
                match_state.pass_target = Some(target);
                match_state.pass_aim = aim;
                telemetry.record(MatchFact::BallReleased {
                    player: possessor,
                    kind: ReleaseKind::Pass,
                    aim,
                });
                release_possession = true;
            }
            OnBallAction::PanicClear => {
                // port of `_AddPanicPass`: blast it forward, away from the middle
                let forward = -side;
                let yside = if player_vel_2d.y >= 0.0 { 1.0 } else { -1.0 };
                let dir_vec = normalized_or_2d(player_vel_2d, Vec2::new(forward, 0.0));
                let away =
                    (normalized_or_2d(dir_vec * Vec2::new(0.8, 1.0), Vec2::new(forward, 0.0))
                        + Vec2::new(forward * 0.7, yside * 0.5))
                    .normalize_or_zero();
                let kick_dir = Vec3::new(away.x, away.y, tuning.clearance.lift).normalize_or_zero();
                kick(
                    &mut ball,
                    &mut ball_position,
                    kick_dir * tuning.clearance.power,
                    Vec3::ZERO,
                    &mut touched_writer,
                );
                match_state.pass_target = None;
                telemetry.record(MatchFact::BallReleased {
                    player: possessor,
                    kind: ReleaseKind::Clearance,
                    aim: player_pos_2d + away * 30.0,
                });
                release_possession = true;
            }
            OnBallAction::Dribble => {
                if !can_knock_on {
                    return;
                }
                // knock-on along the force-field direction; the ball rolls free
                // and the player chases it (no glue). The knock pace follows the
                // carrier's CURRENT speed (as the original ties touches to the
                // desired velocity): a top-speed-based knock in traffic rolls
                // the ball 3+ m ahead, releases possession and gifts it to the
                // opposing designated player — the stealing-metronome bug.
                let all_players: Vec<(TeamId, Vec2, Vec2)> =
                    snaps.iter().map(|s| (s.team(), s.pos, s.vel)).collect();
                let dir_2d =
                    dribble_direction(player_pos_2d, player_vel_2d, player_team, &all_players);
                // In traffic the touch shortens to dribble pace so the ball
                // stays within control range (the original's force-field
                // density lowers the desired velocity the same way); in space
                // it follows the carrier's speed.
                let nearest_opp = snaps
                    .iter()
                    .filter(|s| s.team() != player_team)
                    .map(|s| s.pos.distance(player_pos_2d))
                    .fold(f32::MAX, f32::min);
                let knock_speed = if nearest_opp < contest.knock_on_traffic_distance {
                    crate::team_tactics::DRIBBLE_VELOCITY
                } else {
                    (player_vel_2d.length().max(2.0) + 1.0).min(player_speed + 1.0)
                };
                let knock = Vec3::new(dir_2d.x, dir_2d.y, 0.0) * knock_speed;
                kick(
                    &mut ball,
                    &mut ball_position,
                    knock,
                    Vec3::ZERO,
                    &mut touched_writer,
                );
                telemetry.record(MatchFact::BallReleased {
                    player: possessor,
                    kind: ReleaseKind::DribbleKnock,
                    aim: player_pos_2d + dir_2d * 3.0,
                });
            }
        }

        if let Ok((.., mut player_state, _)) = player_query.get_mut(possessor_body) {
            player_state.last_touch_at = now;
        }
        if release_possession {
            match_state.possession_player = None;
        }
    }
}

/// A controlled first touch when gaining possession: kills most of the ball's
/// momentum so it stays at the feet (stand-in for the original trap animations).
/// The damping matters: a hot trap (ball faster than the player) makes contested
/// duels unresolvable — the ball keeps escaping the 350 ms kick window.
#[allow(clippy::too_many_arguments)]
fn control_touch(
    ball: &mut Ball,
    ball_position: &mut Position,
    toucher: Toucher,
    now: Duration,
    contest: &ContestTuning,
    player_query: &mut Query<BallSystemBody, Without<Ball>>,
    touched_writer: &mut MessageWriter<BallTouched>,
    telemetry: &mut MatchTelemetry,
) {
    let Toucher { player, body } = toucher;
    // A controlled touch is DIRECTED: the ball is set up towards where the
    // carrier wants to go (force-field dribble direction, which is repelled by
    // the pitch lines), not towards wherever he happened to be running — a
    // trap in the raw approach direction next to the sideline knocks the ball
    // straight out and chains endless throw-ins.
    let trap_momentum = if let Ok((_, position, _, _, _, velocity)) = player_query.get(body) {
        let my_pos = position.on_pitch();
        let my_vel = Vec2::new(velocity.0.x, velocity.0.y);
        let all_players: Vec<(TeamId, Vec2, Vec2)> = player_query
            .iter()
            .map(|(_, p_position, p, _, _, v)| {
                (p.id.team, p_position.on_pitch(), Vec2::new(v.0.x, v.0.y))
            })
            .collect();
        let dir = dribble_direction(my_pos, my_vel, player.team, &all_players);
        // set the ball up at dribble pace (AI_GetBallControlMovement): a dead
        // trap parks the ball in the middle of the duel and invites the steal
        let (min_trap, max_trap) = contest.trap_speed_range;
        let speed = (my_vel.length() * contest.trap_speed_from_run).clamp(min_trap, max_trap);
        Vec3::new(dir.x, dir.y, 0.0) * speed
    } else {
        ball.momentum * 0.2
    };
    touch_ball(ball, ball_position, trap_momentum);
    ball.set_rotation(0.0, 0.0, 0.0, 1.0);
    ball.last_touch_team = Some(player.team);
    ball.last_touch_player = Some(player);
    ball.last_touch_at = now;
    if let Ok((.., mut player_state, _)) = player_query.get_mut(body) {
        player_state.last_touch_at = now;
    }
    touched_writer.write(BallTouched { player });
    telemetry.record(MatchFact::Touched {
        player,
        deliberate: true,
    });
}

/// Port of `AI_GetBestDribbleMovement` + `AI_GetForceFieldMovement`
/// (aifunctions.cpp): the carrier is repelled by the 5 nearest opponents
/// (projected 0.25 s ahead) and by the side/back lines, and attracted to the
/// opponent goal (with a center magnet that grows near the backline). Returns
/// the desired dribble direction.
pub fn dribble_direction(
    my_pos: Vec2,
    my_vel: Vec2,
    team: TeamId,
    all_players: &[(TeamId, Vec2, Vec2)], // (team, position, velocity)
) -> Vec2 {
    let side = crate::team_tactics::team_side(team);
    let future_sec = 0.25;
    let offense_factor = 0.75; // 0.7 + dribble_offensiveness/mindset defaults

    // 5 closest opponents
    let mut opponents: Vec<(f32, Vec2)> = all_players
        .iter()
        .filter(|(t, _, _)| *t != team)
        .map(|(_, p, v)| (p.distance(my_pos), *p + *v * future_sec))
        .collect();
    opponents.sort_by(|a, b| a.0.total_cmp(&b.0));
    opponents.truncate(5);

    let near_backline = normalized_clamp(my_pos.x.abs() / 55.0, 0.0, 1.0);
    // near the end of the pitch, we want to get inside again
    let center_modifier_inv = (1.0 - near_backline.powi(2)) * 0.5;
    let opp_goal_pos = Vec2::new(-side * 55.0, my_pos.y * 0.5 * center_modifier_inv);

    struct ForceSpot {
        origin: Vec2,
        repel: bool,
        constant: bool,
        power: f32,
        scale: f32,
        exp: f32,
    }
    let mut force_field = Vec::with_capacity(8);
    for (_, opp_pos) in &opponents {
        force_field.push(ForceSpot {
            origin: *opp_pos,
            repel: true,
            constant: false,
            power: 2.0,
            scale: 10.0,
            exp: 1.0,
        });
    }
    // sideline / backline. Stronger than the original's 4.0: without body
    // shielding the carrier gets herded outward by surrounding opponents, and
    // the lines must win that contest or the ball dribbles out of play.
    force_field.push(ForceSpot {
        origin: Vec2::new(my_pos.x, (36.0 + 5.0) * sign_side(my_pos.y) as f32),
        repel: true,
        constant: false,
        power: 7.0,
        scale: 20.0,
        exp: 0.7,
    });
    force_field.push(ForceSpot {
        origin: Vec2::new((55.0 + 5.0) * sign_side(my_pos.x) as f32, my_pos.y),
        repel: true,
        constant: false,
        power: 7.0,
        scale: 20.0,
        exp: 0.7,
    });
    // love for da goal
    force_field.push(ForceSpot {
        origin: opp_goal_pos,
        repel: false,
        constant: true,
        power: offense_factor,
        scale: 1.0,
        exp: 1.0,
    });

    let current_pos = my_pos + my_vel * future_sec;
    let attractor_damping_distance = 1.0;
    let mut cumul_vec = Vec2::ZERO;
    let mut cumul_force = 0.0;
    for spot in &force_field {
        let distance = spot.origin.distance(current_pos);
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
        Vec2::new(-side, 0.0)
    } else {
        (cumul_vec / cumul_force).normalize_or_zero()
    }
}

/// Finds the earliest spot on the ball's predicted path the player can reach in
/// time, and the estimated arrival time in ms (simplified port of the original
/// `GetTimeNeededToGetToBall` AI support routine).
pub fn find_interception(
    player_pos_2d: Vec2,
    player_speed: f32,
    predictions: &[Vec3],
) -> (Vec2, f32) {
    if predictions.is_empty() {
        return (Vec2::ZERO, f32::MAX);
    }

    // Step time in predictions is 10ms (0.01 seconds)
    let step_time = 0.01;

    for (step_idx, &pred_3d) in predictions.iter().enumerate() {
        let pred_2d = Vec2::new(pred_3d.x, pred_3d.y);
        let dist = player_pos_2d.distance(pred_2d);

        // Time player needs to run to the spot at max speed (adding 0.05s reaction delay)
        let time_to_reach = dist / player_speed + 0.05;
        let ball_arrival_time = step_idx as f32 * step_time;

        if time_to_reach <= ball_arrival_time {
            return (pred_2d, ball_arrival_time * 1000.0);
        }
    }

    // Can't intercept within the 3s horizon: run to the resting point; the time
    // estimate is the full run there
    let last_pred = predictions[predictions.len() - 1];
    let last_2d = Vec2::new(last_pred.x, last_pred.y);
    let time_ms = (player_pos_2d.distance(last_2d) / player_speed + 0.05) * 1000.0;
    (last_2d, 3000.0f32.max(time_ms))
}
