# Ahora

## Objetivo activo

**Cerrar MVP 1.5 — Consolidación.** Quedan dos puntos (unidades y
allocations); los seis criterios de terminado ya se cumplen. Después:
**MVP 2 — partido reglamentariamente completo**.

Estado: **MVP 0 y MVP 1 cerrados** (`REVISION_2026-07-30.md`), **MVP 1.5 en su
último tramo** (`CIERRE_MVP_1_5.md` lleva la cuenta de qué se hizo y por qué).

## Lo que falta de MVP 1.5

### 1. Unidades explícitas (ley 9) — pendiente

El kernel sigue midiendo el tiempo en `u64` de milisegundos:
`Ball.last_touch_time_ms`, `MatchState.period_elapsed_ms`,
`last_possession_change_time`, `set_piece_timer: f32`, y los `now_ms` que cada
sistema recalcula desde `Time`. `Duration` ya entró en el escenario, el reloj
y `PlayerMatchState.last_touch_at`; falta el resto.

Es el punto más invasivo que queda porque toca los cooldowns de toque y de
posesión, que son gameplay calibrado. Hacerlo con la métrica de
`long_match_stats` delante: si los 205 cambios de posesión se mueven, la
conversión cambió el juego.

Considerar además newtypes para metros y m/s donde aclaren.

### 2. Allocations por tick (ley 14) — pendiente

18 `collect()`/`vec!` por tick entre `player_decisions`, `team_tactics` y
`player_movement` (eran 25; el resto se fue con la identidad). Los
`Vec<PlayerReading>` se reconstruyen en cada sistema y en cada toque. La ley lo
prohíbe explícitamente. No es urgente para el resultado, sí para el coste de un
MVP 6 que corra muchas variantes de la misma situación.

## Criterio de terminado de MVP 1.5

1. ✅ Ningún `Entity` como identidad de dominio en APIs públicas.
2. ✅ Ningún atributo de jugador sin mecanismo que lo lea.
3. ✅ Ningún campo de diagnóstico dentro de `MatchState`.
4. ✅ Ningún nombre heredado del original en APIs.
5. ✅ `cargo clippy --all-targets -- -D warnings` limpio.
6. ✅ Una corrida headless emite su forense desde el subsistema, sin código de
   test ad hoc.

## Hecho

### MVP 0 — Constitución

- Norte separado del port; port reclasificado como referencia histórica en
  `references/gameplay_football/`.
- Leyes arquitectónicas, vocabulario, inventario IFAB 2026/27 y estrategia de
  validación.
- CodeGraph evaluado; instalación pendiente de un piloto A/B autorizado.

### MVP 1 — Kernel observable

- **Estado espacial de dominio**: `Position` (metros, Z-up), `Facing` y
  `Velocity` en `crates/domain/src/spatial.rs`. `Transform` ya no es verdad.
- **Setup autoritativo** en `crates/simulation/src/match_setup.rs`: pelota y dos
  onces sin meshes, materiales ni `Visibility`.
- **Presentación como consumidor**: `crates/presentation/src/visuals.rs` crea
  una entidad desechable por cuerpo con `VisualOf` e interpola entre las dos
  últimas posiciones del tick fijo.
- **Fronteras como crates**: domain / simulation / presentation + paquete raíz
  como app. Domain y simulation dependen de subcrates de Bevy y **no** de
  `bevy`, así que la ley 1 la impone Cargo (`cargo tree` no tiene
  `bevy_render` bajo el kernel).
- **Escenarios** (`crates/domain/src/scenario.rs`) y **`ScenarioRunner`**
  (`src/lib.rs`) con `headless` y `with_primitives`.
- **Catálogo de escenarios** (`src/scenarios.rs`), corridos por
  `tests/scenarios.rs`.
- **Overlays diagnósticos** (`crates/presentation/src/overlays.rs`), con la
  geometría como funciones puras con tests.
- **El árbitro publica lo que juzgó** (`OffsideRecords.judged_line_x`).
- **Reloj y fases** (Ley 7) en `crates/simulation/src/match_clock.rs`.
- **HUD** (`crates/presentation/src/hud.rs`).

### MVP 1.5 — Consolidación (lo hecho)

Detalle y razones en `CIERRE_MVP_1_5.md`. En resumen:

- **Identidad de dominio** (`crates/domain/src/identity.rs`): `PlayerId
  { team, shirt }`, `TeamId`, `ByTeam<T>` y `PlayerRegistry`. `Entity` dejó de
  ser memoria persistente en los ocho sitios que lo usaban así.
- **`Player` separado en cuatro** (`crates/domain/src/player.rs`): identidad e
  instrucción, `Attributes` (capacidad), `Mentality` (disposición),
  `PlayerMatchState` (lo que el partido escribe). `PlayerRole` se partió en
  `PlayingPosition` y `TacticalRole`.
- **Subsistema de diagnóstico**: hechos tipados (`MatchFact`),
  `MatchTelemetry` (stream por tick), `MatchLedger` (lo que suman),
  `MatchSnapshot` (el presente, en `domain`, con dos sinks), `render_pitch`
  (el campo en ASCII) y un hub F1 donde overlays y canales son una sola lista.
  Contrato en `DIAGNOSTICS.md`.
- **Nombres**: `player_decisions` y `team_tactics` en vez de `eliza` y
  `team_ai`; `PlayerReading`, `DecisionContext`, `TeamShape`.
- **Higiene**: clippy limpio con `-D warnings`, `Scenario::contradictions()`,
  escenario de red lateral y el fin de partido como estado real.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Sin cambio de mitades** (Ley 8): los equipos defienden el mismo lado los dos
  tiempos. Es lo primero de MVP 2 y toca toda la IA, que asume que el local
  defiende -x. `TeamSide` ya existe en el dominio, sin usar, esperándolo.
- **Sin tiempo añadido** (Ley 7).
- **Kick-off es reanudación nominal**, no regla con posiciones y balón en juego.
- `Ball.predictions` es verdad compartida, no creencia individual: cuando llegue
  MVP 4 deja de ser válido que todos lean el futuro real. `PlayerReading` es
  omnisciente por la misma razón.
- `SimulationSet` refleja el orden del original salvo `MatchLifecycle`; el
  pipeline semántico de `ARCHITECTURE.md` sigue pendiente.
- Árbitro parcial: sin faltas, ventaja ni disciplina.
- `PLAYER_HEIGHT` es una constante que se copia a `Attributes`, no un dato por
  jugador.
- De los overlays del norte faltan los que no tienen dato: campo visual,
  observaciones y edad de memoria (MVP 4), y responsabilidades tácticas más allá
  de la designación (MVP 5).
- Los canales `Formation` y `Performance` existen en el hub y casi no tienen
  productores: `Formation` sólo emite carreras de ataque, `Performance` nada.
- **Nada visual ha sido verificado por nadie.** Bajo Wayland el compositor no
  entrega frames al proceso lanzado desde el shell del agente; hay que correr
  `env -u WAYLAND_DISPLAY ./target/debug/gameplayfootball` y la confirmación es
  del usuario. Con MVP 1.5 hay más que mirar que antes: el HUD dibuja el
  snapshot y F1 abre el hub.

## Reparto previsto de atributos de jugador

Regla de admisión: **un atributo entra cuando tiene mecanismo que lo lee,
unidad real y referencia que lo calibra.** Por eso `stamina`, `acceleration` y
`agility` salieron en MVP 1.5 en vez de quedarse como decoración.

- **MVP 3** — motores: velocidad punta, aceleración, frenado, giro, fatiga,
  alcance.
- **MVP 4** — perceptivos: campo visual, latencia, atención, memoria.
- **MVP 5** — tácticos: familiaridad, disciplina, riesgo.
- **MVP 6** — los vuelve editables para comparar variantes.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
- No añadir un atributo de jugador sin mecanismo, unidad y referencia.
- No añadir un canal de diagnóstico encendido por defecto.
