# Ahora

## Objetivo activo

**MVP 1.75 — Calibración y propiedades.** El trabajo de fondo al retomar.

MVP 1.5 destapó el problema: **el modelo marca 51 goles cada 90 minutos**
(rango 27-81 sobre diez semillas) contra ~2,7 de un partido real
(`REVISION_2026-07-30-reloj.md`). Calibrarlo estaba previsto para MVP 3, pero un
simulador a un orden de magnitud de la única referencia externa trivial de
conseguir no puede demostrar de forma convincente ninguna regla intermedia. Y
no hay ni un test que lo note: los que existen validan reglas IFAB, no
plausibilidad.

Estado: **MVP 0, 1 y 1.5 cerrados** (`REVISION_2026-07-30.md`,
`CIERRE_MVP_1_5.md`), salvo un punto de higiene abajo.

## MVP 1.75, en orden

El instrumental está tomado de OpenFootManager; qué se toma, qué no y por qué
está en `REFERENCIA_OPENFOOTMANAGER.md`. Su motor (cinco zonas, acciones por
minuto) **no** es transferible; su aparato de validación sí.

### 1. Las envolventes son dato, no literales ✅

Hecho el 2026-07-30. `MatchTuning` vive en `crates/domain/src/tuning.rs`,
versionado con `TuningVersion::PortBaseline`, viaja en el `Scenario` (como la
semilla) y se instala como recurso desde `match_setup`. Siete grupos: disputa,
posesión, pase, despeje, tiro, defensa y portería.

La extracción se verificó con la envolvente: **las diez semillas dan números
idénticos** antes y después, en goles, cambios de posesión, racha más larga y
tocadores. Un refactor de parámetros que cambia el resultado es un refactor con
un error dentro, y esa es la única forma de saberlo.

Fuera quedaron, a propósito: las velocidades y los pesos del campo de fuerzas
(`team_tactics.rs`), que son el modelo motor de MVP 3, y las constantes de
física del balón, que se calibran contra vuelo y no contra estadística.

### 2. La envolvente, como herramienta ✅

Hecho el 2026-07-30. `crates/simulation/src/envelope.rs`: `EnvelopeSpec` dice
qué situación y bajo cuántas semillas, `EnvelopeReport::run` la corre y
`Distribution` cuenta lo que una media esconde. Dos usos previstos:
`comparing_builds()` (diez semillas de diez minutos, barato, para comparar dos
builds) y `against_the_real_game(n)` (partidos completos, lo único comparable
con la distribución real).

`render_against_poisson` dibuja el histograma observado junto al de una Poisson
de la media real, porque dos modelos con la misma media y distinta forma solo se
distinguen ahí. Sobre ventanas de menos de 45 minutos el informe **no** dibuja
histograma y dice por qué: escalar diez minutos por nueve conserva la media y
destruye la forma.

El ledger tuvo que aprender a contar lo que no contaba: tiros, tiros a puerta,
pases, pases perdidos y tiempo de posesión por equipo. "A puerta" se mide sobre
la trayectoria que la física acaba de calcular para ese disparo — el diagnóstico
corre después de `BallPhysics`, así que la predicción ya es la del tiro — y
significa "iba dentro si nadie la toca", que es una propiedad del golpeo y no de
lo que hizo la defensa.

**Lo que la primera medición destapó** (diez semillas de diez minutos, tuning
`PortBaseline`):

| Métrica | Modelo | Real |
|---|---|---|
| goles/90 min | 51,3 (27-81) | ~2,7 |
| tiros/90 min | 68 | ~25 |
| tiros a puerta | **100 %** | ~33 % |
| pases/90 min | 1732 | ~800-900 |
| pases completados | **11 %** | ~80 % |

Los dos números en negrita son nuevos y valen más que el ritmo de gol para
saber por dónde empezar a girar. El 100 % dice que el tirador no falla nunca:
apunta a un punto entre los palos con una dispersión de ±1 m, así que lo único
que puede evitar el gol es que alguien se interponga. El 11 % dice que el
partido es un intercambio de pérdidas, no una posesión.

### 2.5. Partir `player_kick_system` antes de girar nada ✅

Hecho el 2026-07-30. Eran 461 líneas y doce parámetros haciendo cinco cosas.
Ahora:

- `crates/simulation/src/ball_contest.rs` — de quién es el balón, en cuatro
  sistemas encadenados: `release_escaped_ball`, `select_ball_challenger`,
  `resolve_tackle`, `collect_loose_ball`. El contacto se publica como hecho del
  tick (`BallContest`) en vez de recalcularse.
- `crates/simulation/src/ball_release.rs` — qué hace con él quien lo tiene:
  `execute_on_ball_action`, que delega el golpeo en `solve_shot`, `solve_pass`,
  `solve_clearance` y `solve_knock_on`. Son **funciones puras**: reciben la
  situación y el tuning, devuelven un `Kick`, y se prueban sin levantar un
  partido. Son exactamente las que el paso 3 va a girar.
- `player_movement.rs` se queda con los cuerpos: designación, integración y
  separación. Bajó de 841 a 302 líneas.

El orden vive en `BallTouchSet::{Contest, Release}`, así que las faltas de
MVP 2 entran como sistema propio en el set de la disputa sin editar nada de
esto.

La división se validó con la envolvente: los mismos diez números antes y
después.

Un hallazgo, de propina: `solve_clearance` mezcla la dirección de carrera con
un sesgo hacia adelante de 0,7, y un defensa que retrocede **despeja hacia su
propia portería**. Está afirmado como test para que el día que se arregle sea a
propósito.

### 3. Calibrar contra la distribución, no contra la media

Sospechosos, ya con medida detrás: el tirador nunca falla (100 % a puerta), el
umbral de tiro deja disparar desde cualquier sitio, el portero no defiende de
verdad, y no hay faltas que interrumpan. El 11 % de pases completados apunta a
que la disputa del balón está rota antes que la puntería.

Registrar antes/después de cada giro (ley de `VALIDATION.md`: separar
calibración de validación, versionar parámetros, no mejorar una métrica
ocultando regresiones).

### 4. Tests de propiedad causal

Los que faltan y son el norte del proyecto: afirmar **dirección de efecto sobre
N corridas**, no valores exactos. Por ejemplo: subir la línea defensiva aumenta
los fueras de juego; presionar alto sube las recuperaciones en campo rival; dos
equipos iguales terminan parejos.

Son inmunes al caos que nos mordió con el reloj: una propiedad afirmada sobre
cien corridas sobrevive a una perturbación de 1 ms; un marcador 1-0 no.

**Criterio de terminado:** el ritmo de gol dentro de lo defendible, el
histograma parecido al real, y al menos tres propiedades tácticas afirmadas
como test.

## Lo que quedó pendiente de MVP 1.5

**Allocations por tick (ley 14).** 18 `collect()`/`vec!` por tick entre
`player_decisions`, `team_tactics` y `player_movement` (eran 25; el resto se fue
con la identidad). Los `Vec<PlayerReading>` se reconstruyen en cada sistema y en
cada toque. No es urgente para el resultado, sí para el coste de un MVP 6 que
corra muchas variantes de la misma situación — y para MVP 1.75, que va a correr
cientos de partidos por medición.

## Criterio de terminado de MVP 1.5

1. ✅ Ningún `Entity` como identidad de dominio en APIs públicas.
2. ✅ Ningún atributo de jugador sin mecanismo que lo lea.
3. ✅ Ningún campo de diagnóstico dentro de `MatchState`.
4. ✅ Ningún nombre heredado del original en APIs.
5. ✅ `cargo clippy --all-targets -- -D warnings` limpio.
6. ✅ Una corrida headless emite su forense desde el subsistema, sin código de
   test ad hoc.

Extra no previsto: el tiempo del kernel es `Duration` en vez de `u64` de
milisegundos, lo que destapó que el reloj anterior perdía 1 ms en el 1 % de los
ticks y que el modelo estaba apoyado en ese ruido.

## Hecho

### Publicación (2026-07-30)

El proyecto dejó de vivir solo en local: `github.com/Uneven3/simulador-futbol`,
con `README.md`, licencia GPLv3 y los cuatro manifiestos sin herencia del
workspace `uneven` (versiones y `bevy` explícitos), para que un clon compile
sin el workspace compartido. Lo único que le falta a un clon es declararse
workspace de sí mismo, y el README dice cómo; ese bloque no se commitea porque
Cargo rechaza que un miembro sea raíz de otro workspace.

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
- **Tiempo como `Duration`** en todo el kernel, y `seeded_envelope` como la
  forma de comparar builds: diez semillas reportadas como tasas, en vez de una
  trayectoria que cualquier perturbación cambia.
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
- **El ritmo de gol es irreal**: 51/90 min contra ~2,7 reales
  (`REVISION_2026-07-30-reloj.md`). Ningún resultado de este simulador puede
  presentarse como predicción hasta calibrarlo.
- **Lo visual ya está verificado** (`VERIFICACION_VISUAL_2026-07-30.md`): HUD,
  hub, overlays y campo funcionan. Lo que falta es de calidad, no de existencia:
  la cámara ve una fracción del campo (lo más molesto: impide juzgar la forma
  del bloque, que es para lo que existe la ventana), el HUD no tiene fondo, no
  hay meshes de portería y el marcador de orientación no se distingue.

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
