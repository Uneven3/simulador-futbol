# Referencia histórica — port de ElizaController

> El nombre y arquitectura Eliza no se usan en APIs nuevas. Se conserva
> trazabilidad con el original. Ver `../../DOMAIN_MODEL.md`.

# Port del ElizaController (IA de jugadores) — 2026-07-11

Estado: **completado y validado headless**. Este documento existe para que
cualquier agente/persona pueda retomar el trabajo sin el contexto de la sesión.

## Qué se portó y a dónde

Fuente C++: `github.com/BazkieBumpercar/GameplayFootball` (clonar a un directorio
temporal para comparar; NO está vendorizado en este repo).

| C++ original | Módulo Rust | Notas |
| :--- | :--- | :--- |
| `onthepitch/teamAIcontroller.cpp` | `src/simulation/team_ai.rs` | Recurso `TeamAis` + sistema `team_ai_update`: possession amounts (fading), línea de offside trap, oponentes peligrosos, man marking (`CalculateMarkingQuality`), attacking runs (cada 500 ms), forward support player (cada 1500 ms), `GetAdaptedFormationPosition`, `ApplyOffsideTrap`. |
| `AIsupport/AIfunctions.cpp::AI_GetAdaptedFormationPosition` | `team_ai.rs::adapted_formation_position` | Mapeo de formación normalizada (-1..1) al bloque dinámico + magnets (yFocus/microFocus/midfieldFocus). |
| `controller/elizacontroller.cpp` | `src/simulation/eliza.rs` | `eliza_movement_system` (movimiento por tick) y `decide_on_ball_action` (decisión con pelota). `GetSupportPosition_ForceField`, `GetLazyVelocity`, on-the-ball (ratings tácticos, pases short/long/high, tiro con odds, panic pass). |
| `controller/playercontroller.cpp` | `eliza.rs` | `AddDefensiveComponent`, `GetDefendPosition`, hunting del portador (2 más cercanos, `huntDistanceThreshold` por mindset). |
| `strategies/offtheball/default_def/mid/off.cpp` | `eliza.rs::off_ball_movement` | Los tres difieren solo en constantes (attackBias min/max, K defensivo, gate de makeRun, trap sí/no) — implementados como un match por rol. |
| `strategies/offtheball/goalie_default.cpp` | `eliza.rs::goalie_movement` | Bisección del ángulo a los palos, lógica de salida (come-out con peligro secundario), intercepción cuando `CalculateIfBallIsBoundForGoal`. |
| `AIfunctions.cpp::AI_CalculateFreeSpace`, `AI_GetMindSet`, `AI_GetForceFieldMovement`, `AI_GetOffsideLine` | `eliza.rs::free_space`, `PlayerRole::mind_set` (data/player.rs), `eliza.rs::force_field_movement`, `eliza.rs::offside_line` | `offside_line` ahora proyecta jugadores con `futureSim_ms` como el original. |
| `player.cpp::_CalculateTacticalSituation` | `eliza.rs::tactical_situation` | forwardSpace/space/forward ratings. |
| `humanoid_utils.cpp::NeedDefendingMovement` | `eliza.rs::need_defending_movement` | |

### Cambios de datos (src/data/)
- `Player`: + `formation_pos: Vec2` (formación normalizada -1..1), `man_marking:
  Option<Entity>`, `avg_velocity: f32` (media ~10 s, para GetLazyVelocity).
- `PlayerStats`: + `work_rate`, `technical_shot`; **`speed` ahora es 8.0**
  (sprintVelocity del original; antes 5.0).
- `MatchRng` (recurso): RNG determinista xorshift para las decisiones con
  `random()` del original (seed fija ⇒ partidos headless reproducibles).
- `Ball::average_position(ms)` (port de `GetAveragePosition`).
- Constantes de gamedefines.hpp en `team_ai.rs` (walk 5.0, sprint 8.0,
  distanceToVelocityMultiplier 2.6, etc.).
- Helpers geométricos en `src/math.rs`: `line_distance_to_point_2d`,
  `line_intersection_2d`, `what_side_2d`, `rotated_2d`, `normalized_or_2d`.

### Ejecución de las decisiones
`player_kick_system` (player_movement.rs) conserva la mecánica de toques
discretos (pickup/tackle/cooldowns) pero la decisión del poseedor ahora viene de
`eliza::decide_on_ball_action` con la prioridad de cola del original:
**panic → pass → shot → dribble**. La ejecución del tiro conserva la receta
tuneada previa (línea de gol en ±55, curl hacia adentro del segundo palo,
topspin 12 — ver memoria de tuning: 40 rad/s de topspin clava los tiros en el
pasto).

## Simplificaciones deliberadas (pendientes conocidos)
- **Sin MentalImage**: los controllers ven el estado real (sin retardo de
  reacción). Portable después si hace falta dificultad ajustable.
- **Sin CalculateDynamicRoles** (asignación húngara de roles): rol dinámico ==
  rol estático.
- **Sin ApplyTeamPressure**: su trigger también está comentado en el original.
- **AI_GetPass** (resolución exacta de dirección/potencia del pase) aproximado
  con las recetas de potencia por tipo (Short/Long/High) ya tuneadas.
- Se mantiene el filtro nuestro de **no pasar a receptores en offside** (el
  original confía solo en el posicionamiento onside del force field).
- El arquero “busca la pelota suelta en su área” con una regla simple (sustituto
  de las anims de pickup/deflect del original).

## Bug del "metrónomo de robos" y su arreglo (2026-07-11, misma sesión)

Reporte del usuario mirando `cargo run`: "no es un partido, son 2 jugadores que
constantemente se quitan la pelota". Cuantificado con el test de estadísticas:
**41,6 cambios de posesión entre equipos por minuto** (uno cada 1,4 s), scrum
rodante en el mediocampo.

Causa raíz: sin las animaciones de control del original no había **shielding**
— cada toque de gambeta era una moneda al aire entre el gambeteador y el
designado rival (llegaban juntos a la pelota suelta), y el quite directo
conectaba en cada ventana de cooldown. Arreglos (player_movement.rs):

1. **Quite con shielding**: el rival solo roba si está más cerca de la pelota
   que el poseedor (×0,8) Y la pelota no está bajo control cercano (>1 m del
   poseedor, o retenida >2 s). Modela el bloqueo corporal del original.
2. **Touch bias en pelota suelta** (port conceptual de `GetLastTouchBias`): si
   el último tocador (≤1,5 s) está a <1 m de la pelota, un rival solo la gana
   llegando 0,25 m más cerca — sin carreras a cara o cruz tras cada toque.
3. **Decisión rápida**: pase/tiro/despeje a los 150 ms de recibir (el original
   encadena trap→pase por la cola de comandos); solo el toque de gambeta
   mantiene la cadencia de 350 ms.
4. **Knock proporcional a la velocidad actual** (no al tope): en tráfico el
   toque es corto (~4,5 m/s); a toda velocidad, largo. Con `speed`=8, el knock
   fijo de 9 m/s soltaba la pelota 3+ m adelante y la regalaba.
5. **Trap con ritmo**: el primer toque orienta la pelota a 2-3,5 m/s en la
   dirección del force field (antes 0,8 m/s: pelota muerta en el duelo).

Resultado (10 min simulados): 18,9 flips/min (antes 41,6), quites directos 17
(antes 190), viaje medio de la pelota entre flips 8,6 m (antes 4,7), racha
máxima de posesión 18 s, 2-2, corners y laterales presentes. El test
`long_match_stats` imprime ahora `flip causes` y el log forense de flips —
usarlo para cualquier tuning futuro de esta zona.

## Validación (2026-07-11)
- `cargo test`: 9/9 pasan. `test_headless_match_flow` ahora mide **crowding**
  (jugadores a <8 m de la pelota, máx. 8) en vez de contar sprinters, porque con
  velocidades reales (walk 5.0) el umbral viejo de 4.5 m/s contaba caminantes.
- `cargo test long_match_stats -- --ignored --nocapture` (10 min simulados):
  score 1-2, 20 tocadores distintos, 1812 toques, 17 disparos, max |ball.x| 58.5.
- **Observación para tuning futuro** (no bloqueante por decisión del usuario de
  completar todo el port primero): la pelota casi no sale del campo (0 laterales
  y 0 corners en 10 min; solo 1 saque de arco). Probablemente el force field
  centra demasiado el juego. Revisar cuando se haga la pasada de tuning.

## Qué sigue (orden acordado con el usuario)
1. **Controles humanos** (gamepad/teclado, cambio de jugador, pase/tiro).
2. Saques ejecutados por jugadores (hoy: pelota colocada, sigue al primer toque).
3. Visual jugable: orientación de jugadores, redes, HUD marcador/reloj.
4. Reloj/tiempos, faltas, audio, menús (Fase 5).
