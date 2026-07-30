use bevy::prelude::*;

pub mod ball_collisions;
pub mod ball_physics;
pub mod eliza;
pub mod player_movement;
pub mod referee;
pub mod team_ai;

pub use ball_collisions::BallCollisionPlugin;
pub use ball_physics::BallPhysicsPlugin;
pub use player_movement::PlayerMovementPlugin;
pub use referee::RefereePlugin;

/// Fixed-tick ordering, mirroring the original `Match::Process()` sequence:
/// players move, ball/body collisions resolve, the ball integrates, then the
/// referee rules on the result.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationSet {
    Players,
    Kicks,
    BallCollisions,
    BallPhysics,
    Referee,
}

pub struct SimulationOrderPlugin;

impl Plugin for SimulationOrderPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                SimulationSet::Players,
                SimulationSet::Kicks,
                SimulationSet::BallCollisions,
                SimulationSet::BallPhysics,
                SimulationSet::Referee,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Ball, BallTouched, MatchState, PitchConfig, SetPiece};
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn build_headless_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.insert_resource(Time::<Fixed>::from_hz(100.0));
        app.insert_resource(MatchState::default());
        app.insert_resource(PitchConfig::default());
        app.add_message::<BallTouched>();
        app.add_plugins((
            SimulationOrderPlugin,
            BallPhysicsPlugin,
            BallCollisionPlugin,
            RefereePlugin,
            PlayerMovementPlugin,
        ));
        // one fixed tick (10 ms) per app.update()
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            10,
        )));
        app.world_mut()
            .spawn((Ball::default(), Transform::from_xyz(0.0, 0.0, 0.11)));
        app
    }

    /// ASCII snapshot of the pitch: team 0 = 'o', team 1 = 'x', ball = '@'.
    /// The cheapest way to SEE the tactical shape (block vs scrum) headless.
    fn print_pitch_snapshot(app: &mut App, label: &str) {
        const W: usize = 92; // columns for x in [-56, 56]
        const H: usize = 25; // rows for y in [-37, 37]
        let mut grid = vec![vec![' '; W]; H];
        let to_cell = |x: f32, y: f32| -> (usize, usize) {
            let cx = ((x + 56.0) / 112.0 * (W as f32 - 1.0)).round() as isize;
            let cy = ((y + 37.0) / 74.0 * (H as f32 - 1.0)).round() as isize;
            (
                cx.clamp(0, W as isize - 1) as usize,
                cy.clamp(0, H as isize - 1) as usize,
            )
        };
        // pitch borders
        let (x0, y0) = to_cell(-55.0, -36.0);
        let (x1, y1) = to_cell(55.0, 36.0);
        for x in x0..=x1 {
            grid[y0][x] = '-';
            grid[y1][x] = '-';
        }
        for row in grid.iter_mut().take(y1 + 1).skip(y0) {
            row[x0] = '|';
            row[x1] = '|';
            let (xm, _) = to_cell(0.0, 0.0);
            row[xm] = ':';
        }
        let mut player_query = app
            .world_mut()
            .query::<(&Transform, &crate::data::Player)>();
        for (t, p) in player_query.iter(app.world()) {
            let (cx, cy) = to_cell(t.translation.x, t.translation.y);
            grid[cy][cx] = if p.team_index == 0 { 'o' } else { 'x' };
        }
        let mut ball_query = app.world_mut().query::<(&Ball, &Transform)>();
        if let Ok((_, t)) = ball_query.single(app.world()) {
            let (cx, cy) = to_cell(t.translation.x, t.translation.y);
            grid[cy][cx] = '@';
        }
        println!("--- {label} ---");
        for row in grid {
            println!("{}", row.into_iter().collect::<String>());
        }
    }

    /// Aggregate-statistics run (10 simulated minutes). The simulation is
    /// deterministic but chaotic, so gameplay must be judged on aggregates,
    /// never on a single minute. Run explicitly with:
    /// `cargo test long_match_stats -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn long_match_stats() {
        let mut app = build_headless_app();

        let mut set_piece_counts: std::collections::HashMap<&'static str, u32> =
            std::collections::HashMap::new();
        let mut prev_set_piece = SetPiece::KickOff;
        let mut distinct_touchers = std::collections::HashSet::new();
        let mut max_ball_x = 0.0f32;
        let mut shots = 0u32;
        let mut prev_touch_time = 0u64;
        let mut touches = 0u32;
        // the frozen-duel metronome shows up here: possession flipping between
        // teams every second means there is no football, just mutual stealing
        let mut possession_team_changes = 0u32;
        let mut prev_possession_team: Option<u32> = None;
        let mut longest_possession_streak_ms = 0u64;
        let mut current_streak_start: Option<u64> = None;
        let mut now_ms = 0u64;
        let mut prev_possession_player: Option<Entity> = None;
        let mut flip_causes: std::collections::HashMap<&'static str, u32> =
            std::collections::HashMap::new();
        let mut flip_log: Vec<String> = Vec::new();
        let mut last_flip_ball_pos: Option<Vec3> = None;
        let mut flip_move_sum = 0.0f32;

        for tick in 0..60_000 {
            app.update();

            if tick % 3000 == 2999 {
                print_pitch_snapshot(&mut app, &format!("t = {}s", (tick + 1) / 100));
            }

            let match_state = app.world().resource::<MatchState>();
            let set_piece = match_state.set_piece;
            if set_piece != prev_set_piece && set_piece != SetPiece::None {
                *set_piece_counts
                    .entry(match set_piece {
                        SetPiece::KickOff => "kickoff",
                        SetPiece::GoalKick => "goal_kick",
                        SetPiece::FreeKick => "free_kick",
                        SetPiece::Corner => "corner",
                        SetPiece::ThrowIn => "throw_in",
                        SetPiece::Penalty => "penalty",
                        SetPiece::None => unreachable!(),
                    })
                    .or_insert(0) += 1;
            }
            prev_set_piece = set_piece;
            let (home, away) = (match_state.home_score, match_state.away_score);

            now_ms += 10;
            let team_now = match_state.possession_team;
            let player_now = match_state.possession_player;
            if team_now.is_some() && team_now != prev_possession_team {
                if prev_possession_team.is_some() {
                    possession_team_changes += 1;
                    let start = current_streak_start.unwrap_or(0);
                    longest_possession_streak_ms = longest_possession_streak_ms.max(now_ms - start);

                    // flip forensics: direct steal (tackle) or loose-ball interception?
                    let cause = if prev_possession_player.is_some() {
                        "tackle"
                    } else {
                        "loose"
                    };
                    *flip_causes.entry(cause).or_insert(0) += 1;
                    let ball_pos = {
                        let mut q = app.world_mut().query::<(&Ball, &Transform)>();
                        q.single(app.world()).unwrap().1.translation
                    };
                    let delta = last_flip_ball_pos.map(|p: Vec3| (ball_pos - p).length());
                    if flip_log.len() < 40 {
                        flip_log.push(format!(
                            "t={:>5.1}s {} at ({:>5.1},{:>5.1}) moved {:>5.1}m since prev flip",
                            now_ms as f32 / 1000.0,
                            cause,
                            ball_pos.x,
                            ball_pos.y,
                            delta.unwrap_or(0.0),
                        ));
                    }
                    if let Some(d) = delta {
                        flip_move_sum += d;
                    }
                    last_flip_ball_pos = Some(ball_pos);
                }
                current_streak_start = Some(now_ms);
                prev_possession_team = team_now;
            }
            prev_possession_player = player_now;

            let mut ball_query = app.world_mut().query::<(&Ball, &Transform)>();
            let (ball, transform) = ball_query.single(app.world()).unwrap();
            assert!(
                transform.translation.is_finite(),
                "Ball position is not finite: {:?}",
                transform.translation
            );
            max_ball_x = max_ball_x.max(transform.translation.x.abs());
            if let Some(toucher) = ball.last_touch_player {
                distinct_touchers.insert(toucher);
            }
            if ball.last_touch_time_ms != prev_touch_time {
                prev_touch_time = ball.last_touch_time_ms;
                touches += 1;
                // a "shot" here = a touch that fires the ball goalwards at pace
                let v = ball.momentum;
                let goalward = v.x.abs() > 12.0 && v.length() > 15.0;
                let deep =
                    transform.translation.x.abs() > 25.0 && (transform.translation.x * v.x) > 0.0;
                if goalward && deep {
                    shots += 1;
                }
            }

            let _ = (home, away);
        }

        let match_state = app.world().resource::<MatchState>();
        println!("=== 10 simulated minutes ===");
        println!(
            "score: {} - {}",
            match_state.home_score, match_state.away_score
        );
        println!("set pieces: {set_piece_counts:?}");
        println!("distinct touchers: {}", distinct_touchers.len());
        println!("total touches: {touches}, goalward blasts: {shots}");
        println!("max |ball.x|: {max_ball_x:.1}");
        println!(
            "possession team flips: {possession_team_changes} ({:.1}/min), longest streak: {:.1}s",
            possession_team_changes as f32 / 10.0,
            longest_possession_streak_ms as f32 / 1000.0
        );
        println!(
            "flip causes: {flip_causes:?}, avg ball travel between flips: {:.1}m",
            flip_move_sum / possession_team_changes.max(1) as f32
        );
        let ms = app.world().resource::<MatchState>();
        println!(
            "turnovers by release kind [none, pass, knock, clear/shot]: {:?}",
            ms.turnovers_by_kind
        );
        println!(
            "pass turnovers: {} at reception (<2.5m of aim), {} en route",
            ms.pass_turnovers_near, ms.pass_turnovers_far
        );
        for l in &ms.pass_turnover_log {
            println!("  {l}");
        }
        println!("--- first flips ---");
        for line in &flip_log {
            println!("{line}");
        }
    }

    /// Headless integration test: runs the full simulation (players, kicks,
    /// collisions, ball physics, referee) at 100 Hz without any rendering and
    /// checks that a match actually unfolds: kickoff restart fires, someone
    /// gains possession, the ball gets kicked around and stays on the pitch
    /// (or triggers a proper set piece when it leaves).
    #[test]
    fn test_headless_match_flow() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        app.insert_resource(Time::<Fixed>::from_hz(100.0));
        app.insert_resource(MatchState::default());
        app.insert_resource(PitchConfig::default());
        app.add_message::<BallTouched>();
        app.add_plugins((
            SimulationOrderPlugin,
            BallPhysicsPlugin,
            BallCollisionPlugin,
            RefereePlugin,
            PlayerMovementPlugin,
        ));
        // one fixed tick (10 ms) per app.update()
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            10,
        )));

        app.world_mut()
            .spawn((Ball::default(), Transform::from_xyz(0.0, 0.0, 0.11)));

        let mut kickoff_restarted = false;
        let mut possession_seen = false;
        let mut kick_seen = false;
        let mut max_sprinters = 0usize;
        let mut distinct_touchers = std::collections::HashSet::new();
        let mut min_player_gap = f32::MAX;
        let mut max_ball_x = 0.0f32;

        // simulate 3 in-game minutes: the opening minute is often a slow
        // two-player midfield duel, the flow assertions need a developed match
        for tick in 0..18000 {
            app.update();
            let match_state = app.world().resource::<MatchState>();
            if match_state.set_piece == SetPiece::None {
                kickoff_restarted = true;
            }
            if match_state.possession_player.is_some() {
                possession_seen = true;
            }

            let mut ball_query = app.world_mut().query::<(&Ball, &Transform)>();
            let (ball, transform) = ball_query.single(app.world()).unwrap();
            if ball.last_touch_team.is_some() {
                kick_seen = true;
            }
            if let Some(toucher) = ball.last_touch_player {
                distinct_touchers.insert(toucher);
            }
            max_ball_x = max_ball_x.max(transform.translation.x.abs());
            let pos = transform.translation;
            assert!(
                pos.x.abs() < 70.0 && pos.y.abs() < 50.0 && pos.z > -0.01 && pos.z < 40.0,
                "Ball escaped the play area: {pos:?}"
            );
            assert!(pos.is_finite(), "Ball position is not finite: {pos:?}");

            // Team discipline: with the Eliza controller, off-the-ball players
            // may legitimately sprint back into formation, so the real
            // regression signal is CROWDING — how many players stand within 8 m
            // of the ball. If everyone chases the ball, this count explodes.
            let mut crowders = 0;
            let mut player_query = app
                .world_mut()
                .query_filtered::<&Transform, With<crate::data::Player>>();
            for player_transform in player_query.iter(app.world()) {
                let d = (player_transform.translation - pos).truncate().length();
                if d < 8.0 {
                    crowders += 1;
                }
            }
            if tick > 500 {
                max_sprinters = max_sprinters.max(crowders);
            }

            // Bodies must not superimpose (positional separation stands in for
            // player-player collision). Skip the first seconds of warmup.
            if tick > 500 {
                let mut transform_query = app
                    .world_mut()
                    .query_filtered::<&Transform, With<crate::data::Player>>();
                let positions: Vec<Vec3> = transform_query
                    .iter(app.world())
                    .map(|t| t.translation)
                    .collect();
                for i in 0..positions.len() {
                    for j in (i + 1)..positions.len() {
                        let mut d = positions[j] - positions[i];
                        d.z = 0.0;
                        min_player_gap = min_player_gap.min(d.length());
                    }
                }
            }
        }

        assert!(kickoff_restarted, "Kickoff never restarted play");
        assert!(possession_seen, "No player ever gained possession");
        assert!(kick_seen, "The ball was never touched in 3 minutes");
        assert!(
            max_sprinters <= 8,
            "Too many players crowding the ball at once ({max_sprinters}): the whole team is chasing it"
        );
        assert!(
            min_player_gap > 0.5,
            "Players superimposed (min gap {min_player_gap}): body separation is not working"
        );
        println!(
            "match summary: {} distinct touchers, max |ball.x| = {max_ball_x:.1}",
            distinct_touchers.len()
        );
        // The frozen-duel failure mode shows exactly 2 touchers and the ball
        // pinned near the center spot; healthy openings can legitimately
        // involve few players, but the ball must both circulate and progress.
        assert!(
            distinct_touchers.len() >= 3,
            "The ball never circulated (only {} distinct players touched it): play is stuck in a frozen duel",
            distinct_touchers.len()
        );
        assert!(
            max_ball_x > 15.0,
            "The ball never progressed up the pitch (max |x| = {max_ball_x:.1}): play is stuck in midfield"
        );
    }
}
