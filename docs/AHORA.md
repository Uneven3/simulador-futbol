# Ahora

## Objetivo activo

**MVP 1 — Kernel observable: cerrado.** Campo, balón, equipos, reloj, fases,
gol/fuera y reanudaciones son inspeccionables headless y con primitivas, y cada
transición nace de un escenario ejecutable.

Siguiente: **MVP 2 — Partido reglamentariamente completo**.

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
- Overlays diagnósticos en `crates/presentation/src/overlays.rs`, alternables
  con F1-F5: velocidad, futuro físico de la pelota (su propio buffer de
  predicción), designación de posesión y poseedor, pase en vuelo, línea de
  offside juzgada y punto de reanudación. La geometría son funciones puras con
  tests; los sistemas solo las pasan a `Gizmos`.
- El árbitro publica la línea que juzgó (`OffsideRecords.judged_line_x`) para
  que la presentación muestre la decisión en vez de recalcular la regla.
- Reloj y fases (Ley 7) en `crates/simulation/src/match_clock.rs`, primer paso
  del tick (`SimulationSet::MatchLifecycle`): PreMatch → FirstHalf → HalfTime →
  SecondHalf → FullTime. Las duraciones son `MatchRegulations`, dato de
  competición, así que un escenario juega tiempos de 20 s.
- HUD en `crates/presentation/src/hud.rs`: marcador, reloj de transmisión, fase
  y motivo de la detención, con las conversiones como funciones puras y tests.
- Catálogo ampliado a 10 escenarios: partido corto, gol a alta velocidad, poste,
  travesaño y pelota que se detiene sobre la línea.

## Siguiente corte

Entrada a MVP 2, empezando por lo que MVP 1 dejó declarado como ausente:

1. Cambio de mitades en el segundo tiempo (Ley 8). Hoy los equipos defienden el
   mismo lado los dos tiempos; tocarlo afecta a toda la IA, que asume que el
   local defiende -x.
2. Tiempo añadido (Ley 7): el árbitro debe contabilizar el tiempo perdido.
3. Kick-off como regla, no como reanudación nominal: posiciones y balón en juego.
4. Escenarios como archivo (RON) cuando exista un consumidor real: hoy son datos
   en Rust y no se añadió `serde` sin contrato que lo justifique.
5. Colocación explícita de jugadores en el escenario (`PlayerSetup` solo tiene
   formación por defecto y solo-pelota), requisito de MVP 6.
6. Reemplazar el resto de `SimulationSet` por el pipeline semántico de
   `ARCHITECTURE.md` (ya entró `MatchLifecycle`).

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
- De la lista de overlays del norte faltan los que aún no tienen dato: campo
  visual, observaciones y edad de memoria (MVP 4), y responsabilidades tácticas
  más allá de la designación de posesión (MVP 5).
- El side netting solo tiene frontera unitaria, no escenario ejecutable.
- `MatchState` acumula estado de partido y diagnósticos de IA en un solo
  recurso; el reloj le sumó dos campos más.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
