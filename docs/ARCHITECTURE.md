# Arquitectura

Este documento fija leyes. El detalle vive en módulos, tests y documentos de
dominio.

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

## Leyes

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
`Match::Process()`.

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

