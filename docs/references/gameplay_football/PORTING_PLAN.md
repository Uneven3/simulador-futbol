# Referencia histórica — Gameplay Football: Bevy 0.19 Porting Plan & Status

> Conserva el estado y razonamiento del port inicial. No es el roadmap actual.
> Ver `../../NORTE.md` y `../../ARCHITECTURE.md`.

This document serves as the project status report and architectural blueprint for the C++ to Rust/Bevy 0.19 port.

Original sources used as reference:
* Game: `github.com/BazkieBumpercar/GameplayFootball` (`src/onthepitch/ball.cpp`, `match.cpp`, `referee.cpp`, `gamedefines.hpp`)
* Engine math (Blunted2): vendored in `github.com/google-research/football` under `third_party/gfootball_engine/src/base/math/`

---

## 📌 CURRENT STATUS

1. **Rust Project Initialized & Compiling:**
   * Location: `/home/francisco/Programming/uneven/gameplayfootball`
   * Dependencies: `bevy = "0.19"` only — **no physics engine**, matching the original.
2. **Architecture Established (Data ➡️ Simulation ➡️ Presentation):**
   * **Data Layer ([src/data/](file:///home/francisco/Programming/uneven/gameplayfootball/src/data/)):** Pure data: `MatchState`, `PitchConfig`, `Ball` (momentum, `rotation_ms`, orientation, predictions, history), `Player`, `PlayerStats`, `Velocity`, `BallTouched` message, `OffsideRecords`.
   * **Simulation Layer ([src/simulation/](file:///home/francisco/Programming/uneven/gameplayfootball/src/simulation/)):** 100 Hz `FixedUpdate` (10 ms steps like the original), ordered via `SimulationSet` (Players → Kicks → BallCollisions → BallPhysics → Referee):
     * `ball_physics.rs`: faithful port of `Ball::CalculatePrediction()`/`Process()`. **The analytical integrator IS the real ball physics** — the state at prediction step 1 becomes the actual ball state each tick, so the AI's predictions and the real ball can never diverge. Includes drag, grass friction, bounce + linear bounce, spin↔ground coupling, Magnus effect (engine -X spin convention), woodwork (first step only, i.e. real ball), goal netting, orientation integration and `touches_net`.
     * `ball_collisions.rs`: simplified port of `Match::CheckBallCollisions()` (capsule body instead of animated limb AABBs; original deflection formula, touch-bias windows and cooldowns).
     * `player_movement.rs`: kinematic players (`Velocity` + integration, positional body separation). Ball interaction is touch-based: discrete knock-ons, directed traps and kicks via `touch_ball` — the ball is never glued/teleported. Ported AI pieces from the original: **designated possession player** (`GetTimeNeededToGetToBall_ms`), **pass ratings** (`AI_GetPassRatings` / `AI_CalculatePassingOdds`: graduated lane odds with opponents projected 0.3 s, `odds^0.8 · pos^0.7`, body-direction penalty), **dribble force field** (`AI_GetBestDribbleMovement` / `AI_GetForceFieldMovement`: repelled by 5 nearest opponents and the lines, drawn to goal with center magnet), **offside line awareness** (`AI_GetOffsideLine`) for pass targets, long balls, striker positioning on the shoulder of the last defender, and the rule that a player flagged offside does not play the ball. Anticipating goalkeeper (covers the predicted crossing point).
     * `referee.rs`: swept goal detection (`CheckForGoal` port: segment vs plane at ±(pitchHalfW + lineHalfW + 0.11), side-netting disallow), whole-ball out-of-play thresholds, exact restart spots (corner at the corner, goal kick at 0.92·halfW, throw-in on the line), offside (port of `Referee::BallTouched()` + `AI_GetOffsideLine`).
   * **Presentation Layer ([src/presentation/](file:///home/francisco/Programming/uneven/gameplayfootball/src/presentation/)):** rendering only. Grass plane with explicit +Z normal (game is Z-up), stadium lighting, broadcast camera. Ball orientation comes from the simulation's integrated quaternion.
3. **Tests (cargo test):**
   * `test_gravity_and_bounce`, `test_magnus_effect` (physics sanity).
   * `test_process_matches_prediction`: the real ball trajectory equals the AI prediction — the core invariant inherited from the original.
   * `test_rotation_multiplied_by_negative_neighborhood`: quaternion scaling takes the short path (`GetRotationMultipliedBy` port).
   * Referee: whole-ball goal line, no tunneling at high shot speed, side-netting disallow, over-the-bar.

---

## 📐 ARCHITECTURAL BLUEPRINT (OOP ➡️ ECS)

| C++ Class | Bevy ECS Concept | Description / Components |
| :--- | :--- | :--- |
| `class Match` | **Resource** | `MatchState` (score, set pieces, possession, restart position). |
| `class Ball` | **Entity + system** | `Ball` component holds momentum/rotation/predictions; `ball_process` (FixedUpdate, 100 Hz) is the ported `Ball::Process()`. No physics engine. |
| `Ball::Touch()` | **Helper fn** | `simulation::ball_physics::touch_ball` — every contact is a discrete momentum change. |
| `Match::CheckBallCollisions()` | **System** | `ball_collisions::ball_body_collisions`. |
| `class Player` | **Entity** | `Player`, `PlayerStats`, `Velocity` (kinematic, integrated manually). |
| `class Referee` | **Systems** | `referee_system` (goals/outs, swept), `referee_offside_system` (reads `BallTouched` messages), `referee_set_piece_system` (restarts). |
| `ElizaController` | **Systems** | `eliza::eliza_movement_system` + `eliza::decide_on_ball_action` — real port (strategies, goalie, defense, on-the-ball). See `eliza-controller-port.md`. |
| `TeamAIController` | **Resource + system** | `team_ai::TeamAis` / `team_ai_update`: offside trap, adapted formation positions, man marking, attacking runs. |
| `HumanController` | **System (pending)** | Phase 4. |

---

## 🚀 STEP-BY-STEP ROADMAP

### Phase 1: Headless Simulation & Math (COMPLETED)
* Ported math helpers (`src/math.rs`): curve, clamps, Blunted2 quaternion Euler conventions, `GetRotationMultipliedBy` (short-path), `GetRotationTo`, `GetRotationAngle`.
* Faithful `Ball::CalculatePrediction()` port as the single source of truth for ball motion (100 Hz fixed timestep).

### Phase 2: Bevy 3D Renderer Setup (COMPLETED)
* 3D camera, lighting, shadow maps; procedural pitch (grass + lines, Z-up).

### Phase 3: Skeletal & Scaled Mesh Setup (Next Steps)
* Load converted player body part meshes; entity hierarchy per player.
* **Implement Body Scaling:** scale limb transforms from player height/weight.
* Visual goal nets + net deformation from `Ball::touches_net`; ball touch / goal post audio (hooks already in place).

### Phase 4: Inputs and AI (AI DONE, inputs next)
* **DONE (2026-07-11):** the real `ElizaController` + `TeamAIController` port replaced the placeholder AI — off-the-ball strategies per line, goalkeeper (goalie_default), man marking, hunting, offside trap, lazy velocity, support force field, and the on-the-ball decision (panic → pass → shot → dribble). Details and deliberate simplifications: `eliza-controller-port.md`.
* Port human control input systems (Gamepad and Keyboard).
* Set pieces actually taken by a player (currently: ball placed, play resumes on pickup).
* **Physics decision (settled):** hybrid — the ball keeps the analytical integrator (the AI depends on prediction == reality); Avian gets reintroduced in Phase 3-4 for player bodies, stadium colliders, `SpatialQuery` and collision events (ball as a sensor collider driven by the integrator).

### Phase 5: Game Loop, Audio & Menus
* Fouls, cards, penalties (offside is already in), match clock and halves.
* Audio via Bevy's audio engine; UI via Bevy UI.
