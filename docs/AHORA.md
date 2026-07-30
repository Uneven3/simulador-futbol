# Ahora

## Objetivo activo

**MVP 1.5 — Consolidación.** No añade capacidades: paga lo que hoy es barato y
después no, y monta el instrumental para poder medir lo que venga.

Antes de esto: **MVP 0 y MVP 1 están cerrados** (ver "Hecho" y
`REVISION_2026-07-30.md`). Después de esto: **MVP 2 — partido
reglamentariamente completo**.

## Por qué existe MVP 1.5

Cada punto de abajo cuesta poco hoy y mucho en cada MVP que pase. Cambiar la
identidad de un jugador con 22 jugadores fijos es un refactor; hacerlo con
sustituciones, replay y escenarios serializados encima es un rediseño.

## Trabajo, en orden

### 1. Identidad de dominio (ley 10)

`Entity` se usa hoy como identidad persistente en ocho sitios:
`MatchState.possession_player`, `previous_possessor`, `pass_target`,
`Player.man_marking`, `Ball.last_touch_player`, `OffsideRecords.players`,
`PossessionDesignation.designated` y el mensaje `BallTouched`.

Introducir `PlayerId` y `TeamId` como newtypes, y dejar `Entity` como lo que la
ley dice que es: transitorio. Sin esto, las sustituciones de MVP 2 y cualquier
escenario serializado rompen.

### 2. Unidades explícitas (ley 9)

`u64` de milisegundos y `f32` sin unidad en las APIs del kernel. `Duration` ya
entró con el escenario y el reloj; extenderlo a los tiempos de posesión, toque y
cooldown. Considerar newtypes para metros y m/s donde aclaren.

### 3. Separar lo que el port dejó junto

- **`PlayerStats`** mezcla capacidad estable (velocidad, aceleración), condición
  variable (fatiga) y mentalidad (work rate). Separar en tres, porque envejecen
  distinto y MVP 3 va a llenar la primera y MVP 5 la tercera.
- **`Player`** mezcla identidad, rol, estado de partido (último toque, velocidad
  media) y marca.
- **`PlayerRole`** mezcla posición y mentalidad: `ARCHITECTURE.md` pide
  explícitamente `PlayingPosition` separado de `TacticalRole`, y `Goalkeeper` en
  vez de `GK`.
- **`MatchState`** es una bolsa: marcador, reloj, fase, posesión, reanudación y
  diagnósticos de IA. Los diagnósticos salen al subsistema del punto 5.

**Regla de admisión de atributos**, para que no vuelva a pasar lo de los tres
stats muertos: un atributo entra cuando tiene mecanismo que lo lee, unidad real
y referencia que lo calibra. Reparto previsto: MVP 3 los motores (velocidad,
aceleración, frenado, giro, fatiga, alcance), MVP 4 los perceptivos (campo
visual, latencia, atención, memoria), MVP 5 los tácticos (familiaridad,
disciplina, riesgo), MVP 6 los vuelve editables.

### 4. Nombres del dominio actual (ley 16)

`eliza.rs`, `team_ai.rs`, `mind_set`, `PlayerSnap` y los comentarios "port of"
siguen nombrando al original. Renombrar módulos y APIs con cinco commits de
historia es barato.

### 5. Diagnóstico y logs

Ver `DIAGNOSTICS.md` para el contrato completo. En resumen: un snapshot con dos
sinks, un trace de eventos por tick aparte, todo apagado por defecto, cada línea
con su tick, y el subsistema absorbe los `info!` sueltos, los campos de
diagnóstico de `MatchState` y el forense a mano de `long_match_stats`.

### 6. Un solo hub de depuración

Ya hay cinco teclas de función sin jerarquía (F1-F5, overlays). Unificarlas con
los canales del punto 5 en un panel que se pueda listar, antes de que sean doce.

### 7. Higiene

- 19 lints de clippy heredados del port, todos en `crates/simulation`.
- Reducir las 25 allocations por tick de `eliza`/`team_ai`/`player_movement`
  (ley 14): los snapshots de jugadores se reconstruyen en cada sistema.
- `Scenario::kick_off()` con ventana de 90 min puede colgar una suite si alguien
  lo pasa a `assert_scenario_holds`; darle una guarda.
- `Expectations` deja pedir `play_never_stops` junto a `set_pieces`, que es
  contradictorio y nadie lo detecta.
- Escenario de side netting (hoy solo frontera unitaria).
- Estado explícito de partido terminado: hoy en `FullTime` el árbitro deja de
  reanudar pero el integrador de la pelota sigue corriendo.

## Criterio de terminado de MVP 1.5

1. Ningún `Entity` como identidad de dominio en APIs públicas.
2. Ningún atributo de jugador sin mecanismo que lo lea.
3. Ningún campo de diagnóstico dentro de `MatchState`.
4. Ningún nombre heredado del original en APIs.
5. `cargo clippy --all-targets -- -D warnings` limpio.
6. Una corrida headless puede emitir su forense desde el subsistema, sin código
   de test ad hoc.

## Hecho

### MVP 0 — Constitución

- Norte separado del port; port reclasificado como referencia histórica en
  `references/gameplay_football/`.
- Leyes arquitectónicas, vocabulario, inventario IFAB 2026/27 y estrategia de
  validación.
- CodeGraph evaluado; instalación pendiente de un piloto A/B autorizado.

### MVP 1 — Kernel observable

- **Estado espacial de dominio**: `Position` (metros, Z-up; jugador anclado en
  el suelo, pelota en su centro), `Facing` y `Velocity` en
  `crates/domain/src/spatial.rs`. `Transform` ya no es verdad en ninguna parte.
- **Setup autoritativo** en `crates/simulation/src/match_setup.rs`: pelota y dos
  onces sin meshes, materiales ni `Visibility`.
- **Presentación como consumidor**: `crates/presentation/src/visuals.rs` crea una
  entidad desechable por cuerpo con `VisualOf` e interpola entre las dos últimas
  posiciones del tick fijo. Un marcador de orientación hace legible `Facing`,
  que una cápsula esconde.
- **Fronteras como crates**: `crates/domain`, `crates/simulation`,
  `crates/presentation` y el paquete raíz como capa app. Domain y simulation
  dependen de subcrates de Bevy (`bevy_ecs`, `bevy_math`, `bevy_time`,
  `bevy_app`, `bevy_log`) y **no** de `bevy`: verificado con `cargo tree` que no
  hay `bevy_render`/`bevy_pbr`/`bevy_asset` bajo el kernel, así que la ley 1 la
  impone Cargo. Presentation no depende de simulation.
- **Escenarios** (`crates/domain/src/scenario.rs`): estado inicial, semilla,
  edición IFAB, reglamento, ventana y hechos esperados, con
  `ScenarioOutcome::mismatches` para decir en qué falló una corrida.
- **`MatchKernelPlugin`** agrupa el partido autoritativo completo; juego, runner
  headless y runner con primitivas añaden exactamente lo mismo.
- **`ScenarioRunner`** (`src/lib.rs`) con `headless` y `with_primitives`.
- **Catálogo de 10 escenarios** (`src/scenarios.rs`), corridos por
  `tests/scenarios.rs`: minuto inicial, partido corto, gol, gol a alta
  velocidad, poste, travesaño, pelota que se detiene sobre la línea, saque de
  banda, saque de meta y córner.
- **Overlays diagnósticos** (`crates/presentation/src/overlays.rs`), F1-F5:
  velocidad, futuro físico de la pelota, designación de posesión, pase en vuelo,
  línea de offside juzgada y punto de reanudación. La geometría son funciones
  puras con tests.
- **El árbitro publica lo que juzgó** (`OffsideRecords.judged_line_x`) para que
  la presentación muestre la decisión en vez de recalcular la regla.
- **Reloj y fases** (Ley 7) en `crates/simulation/src/match_clock.rs`, primer
  paso del tick (`SimulationSet::MatchLifecycle`): PreMatch → FirstHalf →
  HalfTime → SecondHalf → FullTime. Las duraciones son `MatchRegulations`, dato
  de competición, así que un escenario juega tiempos de 20 s.
- **HUD** (`crates/presentation/src/hud.rs`): marcador, reloj de transmisión,
  fase y motivo de la detención, con las conversiones como funciones puras.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Sin cambio de mitades** (Ley 8): los equipos defienden el mismo lado los dos
  tiempos. Es lo primero de MVP 2 y toca toda la IA, que asume que el local
  defiende -x.
- **Sin tiempo añadido** (Ley 7).
- **Kick-off es reanudación nominal**, no regla con posiciones y balón en juego.
- `Ball.predictions` es verdad compartida, no creencia individual: cuando llegue
  MVP 4 deja de ser válido que todos lean el futuro real.
- `SimulationSet` refleja el orden del original salvo `MatchLifecycle`; el
  pipeline semántico de `ARCHITECTURE.md` sigue pendiente.
- Árbitro parcial: sin faltas, ventaja ni disciplina.
- `PLAYER_HEIGHT` y el radio del cuerpo son constantes, no datos por jugador.
- De los overlays del norte faltan los que no tienen dato: campo visual,
  observaciones y edad de memoria (MVP 4), y responsabilidades tácticas más allá
  de la designación (MVP 5).
- **Nada visual ha sido verificado por nadie.** Bajo Wayland la ventana se crea
  pero el compositor no entrega frames al proceso lanzado desde el shell del
  agente; hay que correr `env -u WAYLAND_DISPLAY ./target/debug/gameplayfootball`
  y la confirmación es del usuario.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
- No añadir un atributo de jugador sin mecanismo, unidad y referencia.
