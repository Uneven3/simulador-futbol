use bevy_app::prelude::*;
use bevy_ecs::prelude::*;

pub mod ball_collisions;
pub mod ball_contest;
pub mod ball_physics;
pub mod ball_release;
pub mod diagnostics;
pub mod envelope;
pub mod force_field;
pub mod goalkeeping;
pub mod locomotion;
pub mod match_clock;
pub mod match_setup;
pub mod measurements;
pub mod player_decisions;
pub mod player_movement;
pub mod referee;
pub mod team_tactics;

pub use ball_collisions::BallCollisionPlugin;
pub use ball_contest::BallContestPlugin;
pub use ball_physics::BallPhysicsPlugin;
pub use ball_release::BallReleasePlugin;
pub use diagnostics::MatchDiagnosticsPlugin;
pub use match_clock::MatchClockPlugin;
pub use match_setup::MatchSetupPlugin;
pub use player_movement::PlayerMovementPlugin;
pub use referee::RefereePlugin;

use football_domain::Scenario;

/// The whole authoritative kernel for one scenario.
///
/// Every consumer — the game, the headless runner, the rendered runner — adds
/// exactly this and nothing else to get a match. It owns the fixed-tick order,
/// so no caller can reorder the pipeline by accident.
pub struct MatchKernelPlugin {
    scenario: Scenario,
    retained_facts: usize,
}

impl MatchKernelPlugin {
    pub fn new(scenario: Scenario) -> Self {
        Self {
            scenario,
            retained_facts: 0,
        }
    }

    /// Keep the last `facts` of the run for examination afterwards. A live
    /// match has no reason to; a run being measured does.
    pub fn retaining_facts(mut self, facts: usize) -> Self {
        self.retained_facts = facts;
        self
    }
}

impl Plugin for MatchKernelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MatchSetupPlugin::new(self.scenario.clone()),
            SimulationOrderPlugin,
            MatchClockPlugin,
            BallPhysicsPlugin,
            BallCollisionPlugin,
            RefereePlugin,
            PlayerMovementPlugin,
            BallContestPlugin,
            BallReleasePlugin,
            MatchDiagnosticsPlugin::retaining(self.retained_facts),
        ));
    }
}

/// Whether there is still a match to play.
fn the_match_is_still_on(match_state: Res<football_domain::MatchState>) -> bool {
    !match_state.phase.is_over()
}

/// Fixed-tick ordering: the match lifecycle first (is there a match to play?),
/// then players move, ball/body collisions resolve, the ball integrates, and the
/// referee rules on the result. The middle of this still mirrors the original
/// `Match::Process()` rather than the semantic pipeline in ARCHITECTURE.md.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationSet {
    MatchLifecycle,
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
                SimulationSet::MatchLifecycle,
                SimulationSet::Players,
                SimulationSet::Kicks,
                SimulationSet::BallCollisions,
                SimulationSet::BallPhysics,
                SimulationSet::Referee,
            )
                .chain(),
        )
        // After the final whistle nobody plays on. The ball is deliberately
        // still integrated: it rolls to a stop, as it does when the whistle
        // catches it mid-flight. What stops is football, not physics.
        .configure_sets(
            FixedUpdate,
            (SimulationSet::Players, SimulationSet::Kicks).run_if(the_match_is_still_on),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::TaskPoolPlugin;
    use bevy_math::prelude::*;
    use bevy_time::{TimePlugin, TimeUpdateStrategy};
    use football_domain::scenario::TICK;
    use football_domain::{Ball, MatchState, Position, Scenario, SetPiece};

    /// A whole match with nothing but a task pool and a clock: this is the shape
    /// every scenario runs in. There is no renderer to leave out — this crate
    /// cannot depend on one (architecture law 1).
    fn build_headless_app() -> App {
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), TimePlugin));
        app.add_plugins(MatchKernelPlugin::new(Scenario::kick_off()).retaining_facts(4096));
        // one fixed tick per app.update()
        app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));
        app
    }

    /// The kernel spawns and runs a full match on its own. Whether it stays free
    /// of render assets is no longer a test here — this crate cannot name a
    /// `Mesh`; see `tests/layer_boundaries.rs` in the app for the visual side.
    #[test]
    fn kernel_runs_a_match_on_its_own() {
        let mut app = build_headless_app();
        for _ in 0..300 {
            app.update();
        }

        let mut ball_query = app.world_mut().query::<(&Ball, &Position)>();
        assert!(
            ball_query.single(app.world()).is_ok(),
            "no ball on the pitch"
        );
        let mut player_query = app.world_mut().query::<(
            &football_domain::Player,
            &Position,
            &football_domain::Facing,
        )>();
        assert_eq!(
            player_query.iter(app.world()).count(),
            22,
            "both teams must be on the pitch"
        );
    }

    /// The referee publishes the line it judged against, so diagnostics can show
    /// the decision instead of recomputing the rule.
    #[test]
    fn the_referee_publishes_the_offside_line_it_judged() {
        use football_domain::OffsideRecords;

        let mut app = build_headless_app();
        let mut judged_line = None;
        for _ in 0..3000 {
            app.update();
            let records = app.world().resource::<OffsideRecords>();
            if let Some(line_x) = records.judged_line_x {
                judged_line = Some((line_x, records.judged_against_team));
                break;
            }
        }

        let (line_x, defending_team) =
            judged_line.expect("no touch in 30 s produced an offside judgement");
        assert!(
            line_x.abs() <= 55.0,
            "the judged line must lie on the pitch, got {line_x}"
        );
        assert!(
            defending_team.is_some(),
            "a judged line without the team it was judged against says nothing"
        );
    }

    /// Aggregate-statistics run (10 simulated minutes). The simulation is
    /// deterministic but chaotic, so gameplay must be judged on aggregates,
    /// never on a single minute. Run explicitly with:
    /// `cargo test long_match_stats -- --ignored --nocapture`
    ///
    /// Everything printed here is read from the diagnostics subsystem. This
    /// test used to run its own possession bookkeeping, its own turnover
    /// attribution and its own ASCII pitch, which meant the numbers a run
    /// reported existed nowhere else and could not be asked for in any other
    /// context.
    #[test]
    #[ignore]
    fn long_match_stats() {
        use crate::diagnostics::{
            DiagnosticChannel, MatchFact, MatchLedger, MatchSnapshot, MatchTelemetry, ReleaseKind,
            render_pitch,
        };

        let mut app = build_headless_app();

        for tick in 0..60_000 {
            app.update();
            if tick % 3000 == 2999 {
                println!("--- t = {}s ---", (tick + 1) / 100);
                println!("{}", render_pitch(app.world_mut()));
            }
            let ball_pos = {
                let mut balls = app.world_mut().query_filtered::<&Position, With<Ball>>();
                balls.single(app.world()).unwrap().0
            };
            assert!(
                ball_pos.is_finite(),
                "Ball position is not finite: {ball_pos:?}"
            );
        }

        let elapsed = app.world().resource::<MatchState>().period_elapsed;
        let ledger = app.world().resource::<MatchLedger>();
        println!("=== 10 simulated minutes ===");
        println!("score: {} - {}", ledger.goals.home, ledger.goals.away);
        println!("distinct touchers: {}", ledger.distinct_touchers());
        println!(
            "possession changes: {} ({:.1}/min), longest spell: {:.1}s",
            ledger.possession_changes,
            ledger.changes_per_minute(elapsed),
            ledger.longest_spell.as_secs_f32()
        );
        println!(
            "changes by cause: {} tackles, {} loose balls",
            ledger.tackles(),
            ledger.loose_balls()
        );
        println!(
            "turnovers by release: pass {}, knock-on {}, clearance {}, shot {}",
            ledger.turnovers_of(ReleaseKind::Pass),
            ledger.turnovers_of(ReleaseKind::DribbleKnock),
            ledger.turnovers_of(ReleaseKind::Clearance),
            ledger.turnovers_of(ReleaseKind::Shot),
        );
        println!(
            "lost passes: {} at reception (<2.5 m of the aim), {} en route",
            ledger.pass_turnovers_near, ledger.pass_turnovers_far
        );

        for line in app.world().resource::<MatchSnapshot>().lines() {
            println!("{line}");
        }

        let telemetry = app.world().resource::<MatchTelemetry>();
        println!("--- restarts awarded ---");
        for recorded in telemetry.recorded_on(DiagnosticChannel::RefereeDecisions) {
            println!("[t{:06}] {}", recorded.tick, recorded.fact);
        }
        println!("--- last lost balls ---");
        for recorded in telemetry
            .history()
            .filter(|r| matches!(r.fact, MatchFact::Turnover { .. }))
            .rev()
            .take(20)
        {
            println!("[t{:06}] {}", recorded.tick, recorded.fact);
        }
    }

    /// The same ten minutes under several seeds, reported as rates.
    ///
    /// One run of a deterministic but chaotic model says nothing: two builds
    /// that differ by a rounding decision produce different matches without
    /// either being worse. What can be compared is the envelope across seeds.
    ///
    /// Cada corrida se anexa a `measurements/envelope.csv` y el test imprime el
    /// delta contra la anterior: lo que hay que leer tras un refactor es esa
    /// tabla, no el informe entero.
    ///
    /// `cargo test --release -p gameplayfootball_simulation seeded_envelope -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn seeded_envelope() {
        use crate::envelope::{EnvelopeReport, EnvelopeSpec};
        use crate::measurements;

        let report = EnvelopeReport::run(&EnvelopeSpec::comparing_builds());
        println!("{}", report.render());

        let log = measurements::default_log();
        match measurements::append(&report, &log) {
            Ok(id) => println!(
                "\n{}\nregistrada como {id} en {}",
                measurements::compare_last_two(&log),
                log.display()
            ),
            Err(e) => println!("\nno se pudo registrar la medición: {e}"),
        }
    }

    /// Goles por partido contra la distribución real, sobre partidos completos.
    ///
    /// Es la medición que decide si MVP 1.75 terminó: la referencia son ~2,7
    /// goles por partido casi como una Poisson, y acertar la media con otra
    /// forma no es acertar. Cuesta un par de minutos por partido, así que se
    /// corre al calibrar, no para comparar builds.
    ///
    /// `cargo test --release -p gameplayfootball_simulation goal_distribution -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn goal_distribution() {
        use crate::envelope::{EnvelopeReport, EnvelopeSpec};

        let report = EnvelopeReport::run(&EnvelopeSpec::against_the_real_game(20));
        println!("{}", report.render());
    }

    /// Headless integration test: runs the full simulation (players, kicks,
    /// collisions, ball physics, referee) at 100 Hz without any rendering and
    /// checks that a match actually unfolds: kickoff restart fires, someone
    /// gains possession, the ball gets kicked around and stays on the pitch
    /// (or triggers a proper set piece when it leaves).
    #[test]
    fn test_headless_match_flow() {
        let mut app = build_headless_app();

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

            let mut ball_query = app.world_mut().query::<(&Ball, &Position)>();
            let (ball, ball_position) = ball_query.single(app.world()).unwrap();
            if ball.last_touch_team.is_some() {
                kick_seen = true;
            }
            if let Some(toucher) = ball.last_touch_player {
                distinct_touchers.insert(toucher);
            }
            let pos = ball_position.0;
            max_ball_x = max_ball_x.max(pos.x.abs());
            assert!(
                pos.x.abs() < 70.0 && pos.y.abs() < 50.0 && pos.z > -0.01 && pos.z < 40.0,
                "Ball escaped the play area: {pos:?}"
            );
            assert!(pos.is_finite(), "Ball position is not finite: {pos:?}");

            // Off-the-ball players may legitimately sprint, so the regression
            // signal is crowding: how many stand within 8 m of the ball.
            let mut crowders = 0;
            let mut player_query = app
                .world_mut()
                .query_filtered::<&Position, With<football_domain::Player>>();
            for player_position in player_query.iter(app.world()) {
                let d = (player_position.0 - pos).truncate().length();
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
                let mut position_query = app
                    .world_mut()
                    .query_filtered::<&Position, With<football_domain::Player>>();
                let positions: Vec<Vec3> = position_query
                    .iter(app.world())
                    .map(|position| position.0)
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
