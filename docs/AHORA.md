# Ahora

## Objetivo activo

Completar **MVP 1 — Kernel observable**: la misma situación debe correr headless
y renderizada, con entidades autoritativas separadas de sus representaciones.

## Hecho

- Norte separado del port.
- Leyes arquitectónicas.
- Vocabulario inicial.
- Inventario IFAB 2026/27.
- Estrategia de validación.
- Port reclasificado como referencia.
- CodeGraph evaluado; instalación pendiente de un piloto A/B autorizado.
- Estado espacial de dominio: `Position` (metros, Z-up), `Facing` y `Velocity`
  en `crates/domain/src/spatial.rs`. `Transform` ya no es verdad en ninguna parte.
- Setup autoritativo del partido en `crates/simulation/src/match_setup.rs`: pelota y dos
  onces sin meshes, materiales ni `Visibility`.
- Presentación como consumidor: `crates/presentation/src/visuals.rs` crea una entidad
  desechable por cuerpo con `VisualOf`, e interpola entre las dos últimas
  posiciones del tick fijo. Cámara y luces separadas.
- Tests headless sin assets.
- Fronteras como crates: `crates/domain`, `crates/simulation`,
  `crates/presentation` y el paquete raíz como capa app. Domain y simulation no
  dependen de `bevy` sino de sus subcrates, así que la ley 1 la impone Cargo: el
  kernel no puede nombrar un `Mesh`. Presentation no depende de simulation.
- `tests/layer_boundaries.rs`: la misma situación headless y con primitivas da
  posiciones idénticas durante 600 ticks, y cada cuerpo tiene exactamente una
  representación.
- Esquema de escenarios en `crates/domain/src/scenario.rs`: estado inicial
  (pelota, jugadores o solo pelota), semilla, edición IFAB, ventana simulada y
  hechos esperados. `MatchSetupPlugin` lo instala; el escenario es la única
  fuente del estado inicial y de la semilla.
- `MatchKernelPlugin`: el partido autoritativo completo en un plugin. Juego,
  runner headless y runner con primitivas añaden exactamente lo mismo.
- `ScenarioRunner` en `src/lib.rs` (capa app) con `headless` y
  `with_primitives`, y catálogo en `src/scenarios.rs`: gol, saque de banda,
  saque de meta, córner y minuto inicial. `tests/scenarios.rs` los corre.

## Siguiente corte

1. Overlays diagnósticos sobre las primitivas: predicción de la pelota,
   designación de posesión, línea de offside.
2. Escenarios como archivo (RON) cuando exista un consumidor real: hoy son datos
   en Rust y no se añadió `serde` sin contrato que lo justifique.
3. Colocación explícita de jugadores en el escenario (`PlayerSetup` solo tiene
   formación por defecto y solo-pelota), requisito de MVP 6.
4. Reemplazar `SimulationSet` por el pipeline semántico de `ARCHITECTURE.md`.

## Deuda observada

- `Player` mezcla identidad, rol, historial, marca y promedio de movimiento.
- `PlayerRole` mezcla posición y mentalidad.
- `Ball.predictions` es verdad compartida, no creencia individual.
- `SimulationSet` refleja el original, no el pipeline de `ARCHITECTURE.md`.
- `eliza`, `team_ai`, `mind_set` y comentarios "port of" dominan APIs.
- Árbitro parcial.
- `cargo clippy -- -D warnings` falla con 19 lints heredados del port, todos en
  `crates/simulation` (`collapsible_if`, `needless_range_loop`,
  `too_many_arguments`); ninguno fue introducido por los cortes de capas.
- `PLAYER_HEIGHT` y el radio del cuerpo son constantes, no datos por jugador.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
