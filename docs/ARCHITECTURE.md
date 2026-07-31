# Arquitectura

Este documento fija leyes. El detalle vive en módulos, tests y documentos de
dominio. El código las cita por § y no se mergea código que las viole; lo que
hoy las viola está listado al final, con nombre y medida.

## Capas

```text
football_domain
      ↑
football_simulation
      ↑             ↖
football_app      football_presentation
```

- **Domain:** tipos, unidades, reglas, hechos, intents y configuración.
  `crates/domain`, paquete `gameplayfootball_domain`, lib `football_domain`.
- **Simulation:** ECS autoritativo, física, percepción, decisión, táctica,
  arbitraje y telemetría. `crates/simulation`, lib `football_simulation`.
- **Presentation:** visuales, cámara, animación, UI, audio y overlays; solo lee.
  `crates/presentation`, lib `football_presentation`.
- **App:** composición, escenario y ciclo de vida. Es el paquete raíz
  `gameplayfootball` (`src/main.rs` y `tests/`).

Son crates, así que Cargo impide las dependencias inversas. Domain y simulation
dependen de subcrates de Bevy (`bevy_ecs`, `bevy_math`, `bevy_time`,
`bevy_app`, `bevy_log`) y **no** de `bevy`: sin `bevy_render`, `bevy_pbr` ni
`bevy_asset` en el grafo, una regla no puede expresarse en términos de un mesh
ni el kernel puede construir geometría. Solo presentation y app ven el motor
completo.

## Leyes de dominio

Qué es verdad en un partido y quién puede decidirlo.

1. Simulación headless: sus tests no registran `Mesh`, `StandardMaterial`,
   `Image`, ventanas ni `AssetServer`.
2. Entidad autoritativa sin visuales. Otra entidad usa
   `VisualOf(simulation_entity)`.
3. Assets sin autoridad: bones, clips y geometría no deciden contacto,
   velocidad ni resultado.
4. Components/Resources/Messages son datos.
5. Un escritor por estado; otros publican intents o hechos.
6. Verdad y conocimiento son distintos; decisiones no leen verdad no observada.
7. Intención y ejecución son distintas; el motor produce una acción alcanzable.
8. Regla y árbitro son distintos; un incidente puede no ser observado.
9. Tiempo/unidades explícitos en APIs.
10. IDs de dominio son newtypes; `Entity` es transitorio.
11. Aleatoriedad inyectada y semilla registrada.
12. Reglas versionadas por edición IFAB y competición.
13. Fidelidad con métrica, referencia y tolerancia.
14. Evitar allocations por tick; reutilizar buffers tras medir capacidad.
15. Rust seguro, enums exhaustivos, APIs pequeñas, `Option` y `Result`.
16. Nombres describen el dominio actual, no Gameplay Football.

## Leyes de ingeniería

Cómo se escribe el código que sostiene lo anterior. Son SOLID traducido a ECS:
en ECS no hay herencia, así que la responsabilidad única se mide por sistema y
el desacople se consigue con componentes y mensajes, no con interfaces.

17. **Una responsabilidad por sistema.** Si el nombre necesita una "y", son dos
    sistemas. Un sistema que gana posesión, resuelve la disputa y ejecuta el
    disparo no se puede calibrar por partes ni testear por partes.
18. **Se extiende agregando, no editando.** Una regla nueva entra como sistema o
    componente nuevo en su `SystemSet`, no como una rama más dentro de un
    sistema existente. Si añadir faltas obliga a editar el sistema del regate,
    la costura está en el sitio equivocado.
19. **~300 líneas por archivo y ~80 por función son señal de dividir**, no un
    bloqueo. Un archivo que crece sin dividirse deja de tener dueño: nadie
    puede decir qué contiene sin leerlo entero.
20. **Un sistema con más de ocho parámetros agrupa en `SystemParam`.**
    `#[allow(clippy::too_many_arguments)]` no es una solución, es deuda
    anotada, y cada uno debe decir por qué sigue ahí.
21. **Los parámetros que fijan el resultado son dato versionado**
    (`MatchTuning`, `MatchRegulations`), con un solo lugar por cada valor por
    defecto. Un número dentro de la lógica no se puede barrer, ni reportar junto
    al resultado que produjo.
22. **La lógica calculable vive en funciones puras**, probables sin `App`. El
    sistema lee, llama y escribe; no calcula geometría ni resuelve reglas en
    línea.
23. **Visibilidad mínima:** `pub` solo lo que otro crate consume de verdad. La
    frontera de un crate es su API, no la suma de sus módulos.
24. **Dependencia nueva:** OK humano previo, y el contrato que resuelve escrito
    en el `Cargo.toml` que la añade.
25. **`cargo fmt` y `clippy -D warnings` antes de dar algo por terminado.** Cada
    `#[allow]` lleva justificación en su línea.
26. **Comentarios para invariantes, restricciones y procedencia del original.**
    Nunca el *qué*: eso lo dice el código. Las citas al C++ original son
    trazabilidad hacia `references/gameplay_football/`, no nombres del presente
    (ley 16).
27. **Ningún comentario ni documento registra una medición, una fecha o el
    relato de una sesión.** Las mediciones van a `measurements/`, la historia al
    mensaje de commit, y el comentario dice el invariante. Un comentario más
    largo que el código que explica está contando una historia; un documento que
    hay que reeditar cada vez que cambia una cifra es una copia de un CSV.

## Pipeline

```text
FixedUpdate
  Match lifecycle
  → World sensing
  → Player observations
  → Belief updates
  → Tactical responsibilities
  → Player intentions
  → Motor planning/commitments
  → Body movement/contacts
  → Ball integration
  → Physical incidents
  → Referee observations/decisions
  → Rule transitions/telemetry

Update
  Snapshot interpolation
  → primitive/skinned visuals
  → overlays, camera, UI and audio
```

El orden se expresa con `SystemSet` semánticos, no con el orden heredado de
`Match::Process()`. Hoy `SimulationSet` sigue siendo el orden del original
(`MatchLifecycle → Players → Kicks → BallCollisions → BallPhysics → Referee`):
la migración a este pipeline es deuda declarada.

## Mapa de módulos

Cada módulo dice qué posee y dónde está su frontera. Lo que no aparece aquí no
tiene dueño, y eso es un bug de arquitectura antes que de código.

| Módulo | Posee | Frontera |
|---|---|---|
| `domain::identity` | `PlayerId`, `TeamId`, `ByTeam`, `PlayerRegistry` | Única traducción identidad ↔ `Entity`; nadie más guarda `Entity` como memoria |
| `domain::match_state` | `MatchState`, `MatchRegulations`, `PitchConfig`, `MatchRng` | El estado del partido; solo el kernel lo escribe |
| `domain::tuning` | `MatchTuning` y sus grupos | Un único hogar por valor por defecto (§21) |
| `domain::scenario` | `Scenario`, `Expectations`, `ScenarioOutcome` | La situación reproducible completa: estado inicial, semilla, ventana y afirmaciones |
| `domain::player` | `Player`, `Attributes`, `Mentality`, `PlayerMatchState` | Identidad e instrucción, capacidad, disposición y lo que el partido escribe, separados |
| `domain::math` | Geometría y RNG puros | Sin Bevy más allá de `bevy_math`; todo testeable sin `App` |
| `simulation::match_setup` | Instalación del escenario y de los cuerpos | Único que hace spawn de entidades autoritativas |
| `simulation::match_clock` | Reloj y fases (Ley 7 IFAB) | Único escritor de `period_elapsed` y `phase` |
| `simulation::team_tactics` | `TeamTactics`, forma del bloque, trampa del fuera de juego | Lee estado, escribe solo su recurso |
| `simulation::player_decisions` | Adónde corre cada jugador y qué hace con el balón | Decide; no ejecuta ni toca el balón |
| `simulation::player_movement` | Designación de posesión, integración de cuerpos y separación | Único escritor de `Position` de los jugadores |
| `simulation::ball_contest` | De quién es el balón: escapadas, contacto, entrada, recogida | Único que otorga la posesión; publica `BallContest` como hecho del tick |
| `simulation::ball_release` | Cómo sale el balón del pie: tiro, pase, despeje, conducción | Ejecuta la decisión ajena; las recetas son funciones puras |
| `simulation::ball_physics` | Integración y predicción del balón | La predicción es la trayectoria futura real; nadie más la calcula |
| `simulation::ball_collisions` | Contacto balón-cuerpo y balón-portería | Emite hechos; no decide reglas |
| `simulation::referee` | Fuera de juego, fuera de banda, gol y reanudaciones | Único que otorga `SetPiece` y cambia el marcador |
| `simulation::diagnostics` | `MatchFact`, `MatchTelemetry`, `MatchLedger`, `MatchSnapshot` | Solo lee estado autoritativo; apagado por defecto |
| `presentation::*` | Visuales, cámara, HUD, overlays, hub de depuración | Solo lee; borrar el crate deja un partido completo |
| `src/` (app) | Composición, catálogo de escenarios, `ScenarioRunner` | Cablea capas; no decide nada de fútbol |

## Nomenclatura

- Sistemas: `update_player_observations`, `select_player_intents`,
  `integrate_ball_motion`, `detect_out_of_play`.
- Funciones puras: `estimate_interception_time`, `evaluate_passing_lane`,
  `classify_offside_position`.
- Hechos: `BallContact`, `PotentialFoul`, `RestartAwarded`.
- Solicitudes: `KickIntent`, `MovementIntent`, `SubstitutionRequest`.
- Evitar `get_` cuando se calcula, estima, clasifica o selecciona.
- `Goalkeeper`, no `GK`; `PlayingPosition` separado de `TacticalRole`.

## Rust y borrow checking

Debe compilar con el checker estable actual. Polonius no sustituye:

- queries pequeñas;
- componentes descompuestos;
- fases de lectura, propuesta y aplicación;
- mensajes para evitar escritores cruzados.

Traits solo para contratos con múltiples implementaciones reales. Estados
cerrados usan enums.

## Pruebas

- unitarias para geometría/unidades;
- escenarios IFAB;
- invariantes por tick;
- simulaciones largas para distribuciones;
- prueba headless sin assets;
- prueba de que presentación no muta estado autoritativo.

## Deuda declarada contra estas leyes

Medido el 2026-07-30, al escribir las leyes de ingeniería. Se registra con
número para que nadie la descubra dos veces.

**§17 y §19 — tamaño y responsabilidad.**

| Sitio | Medida | Qué contiene |
|---|---|---|
| `player_decisions.rs` | 1352 líneas | Campo de fuerzas, movimiento sin balón, portero y decisión con balón |
| `team_tactics.rs` | 896 líneas | `update_team_tactics` (239) y `get_adapted_formation_position` (210) |
| `ball_physics.rs` | 538 líneas | `calculate_prediction` (415), port directo del integrador original |
| `ball_contest.rs` | 473 líneas | Cuatro sistemas de una responsabilidad, pero el archivo ya pide separar *quién* toca de *cómo* se toca |
| `ball_release.rs` | 451 líneas | Un sistema y cuatro recetas puras; un tercio son tests |

Se parten al tocarlos por otra razón; partirlos antes es riesgo sin lector.

**Pagado el 2026-07-30:** `player_kick_system` eran 461 líneas y doce
parámetros haciendo cinco cosas. Hoy son cuatro sistemas encadenados en
`ball_contest.rs` (`release_escaped_ball`, `select_ball_challenger`,
`resolve_tackle`, `collect_loose_ball`) más `execute_on_ball_action` en
`ball_release.rs`, que delega el golpeo en funciones puras (`solve_shot`,
`solve_pass`, `solve_clearance`, `solve_knock_on`) probables sin `App`.
`player_movement.rs` bajó de 841 a 302 líneas. La envolvente dio los mismos
diez números antes y después: el refactor fue fiel.

**§20 — parámetros.** Quedan tres `#[allow(clippy::too_many_arguments)]`
(`team_tactics.rs`, `ball_collisions.rs`, `player_decisions.rs`), de los cinco
que había. Los sistemas del toque usan `SystemParam` derivado (`Touching`,
`MatchSettings`) en vez de sumar parámetros.

**§18 — extensión.** Las faltas de MVP 2 ya tienen dónde entrar sin editar nada:
un sistema propio dentro de `BallTouchSet::Contest`, que es donde ocurre el
incidente.

La división se valida con la envolvente: si las diez semillas dan números
idénticos, el refactor fue fiel (`docs/VALIDATION.md`).
