# Ahora

## Objetivo activo

**MVP 2 — Partido reglamentariamente completo.** MVP 1.75 quedó cerrado el
2026-07-30 (abajo, con lo que destapó), y su primer encargo era investigar dos
defectos medidos antes de añadir reglas. El primero resultó ser un defecto del
árbitro, no de la disputa.

### 1. El signo del fuera de juego estaba invertido ✅

`referee_offside_system` usaba `att_dir = -def_side`, y la dirección de ataque
del que toca es `def_side`. Anotaba en fuera de juego a los jugadores que
estaban **detrás** de la línea. Como un anotado no puede disputar el balón
(`select_ball_challenger` lo salta), el efecto no era pitar sino congelar: **9,4
de 11 jugadores anotados por tick**, y de las 4466 veces en que el receptor de
un pase llegaba a tiro del balón, las 4466 estaba anotado.

| Métrica | Antes | Después | Real |
|---|---|---|---|
| goles/90 min | 51,3 (27-81) | 23,4 (9-36) | ~2,7 |
| tiros/90 min | 68 | 34 | ~25 |
| pases completados | 11 % | 55 % | ~80 % |
| cambios de posesión/min | 19,9 | 14,1 | — |

Sin girar un parámetro. El 11 % de pases nunca fue "falta percepción" (MVP 4),
como decía la tabla de atribución de MVP 1.75: era un signo. El juicio salió a
`judge_offside_positions`, pura, con dos tests que afirman el signo.

### 2. La simetría, roja a propósito ⏳ ← siguiente

`two_identical_teams_finish_level` falla: el sesgo local pasó de 0,62 a 1,00
(7-0). Estaba tapado, no ausente. Causa localizada:
`referee_set_piece_system` recoloca a los dos equipos en su formación base —un
espejo exacto— y suelta el balón sin dárselo a nadie; los dos delanteros salen
a la vez y el empate lo rompen tres desempates que van todos para el local (el
`<=` de `designated_player_overall`, el de `team_tactics`, y el `<` estricto de
`select_ball_challenger`, que se queda con el primero que itera).

Se arregla dando el balón a quien saca, que es la Ley y ya estaba en la deuda
declarada. Después: portero que ataja, faltas, y cambio de mitades.

## Lo que fue MVP 1.75

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

### 3. Cada desvío, atribuido a su causa — sin girar nada ✅

Decidido el 2026-07-30, a pregunta del usuario: **calibrar ahora sería fitting**.
Girar parámetros hasta que salgan 2,7 goles compensaría con números la falta de
mecanismos, y habría que deshacerlo cuando lleguen. El razonamiento está en
`NORTE.md`; la calibración pasa a ser un hito propio después de MVP 4.

Lo medido, y a quién le toca resolverlo:

| Desvío | Modelo | Real | Causa | Se resuelve en |
|---|---|---|---|---|
| Tiros a puerta | 100 % | ~33 % | No existe el error de golpeo: se apunta entre palos con ±0,5 m y nada lo estorba | MVP 3 (motor, fatiga) |
| Pases completados | 11 % | ~80 % | Nadie tiene percepción parcial: el defensor lee la trayectoria real del balón | MVP 4 (percepción) |
| Goles/90 min | 51 | ~2,7 | Consecuencia de los dos anteriores, más que nadie evita el gol | MVP 2 (portero, faltas) |
| Pases/90 min | 1732 | ~850 | El balón cambia de dueño 20 veces por minuto | MVP 3 + MVP 4 |

Ninguno de los cuatro es un parámetro mal puesto. Ese es el resultado del paso 3:
saber que no lo son.

**Salvo dos, que sí son defectos nuestros** y los descubrió el paso 4 al
intentar afirmar propiedades sobre ellos:

- **El alcance del receptor está muerto.** `contest.receiver_trap_reach` existe
  para que un pase se complete contra el marcador que está uno o dos metros por
  detrás, y es lo que aquí sustituye a las animaciones de control del original.
  Pasar de 1,1 m a 3,0 m produce partidos **idénticos bit a bit**: la rama que
  lo lee no se toma nunca en el momento que importa. Sospecha sin confirmar:
  `update_possession_designation` borra `pass_target` cuando el balón baja de
  0,3 m/s, y los pases resueltos llegan agonizando al receptor justo antes de
  la disputa, que corre después. **Es el candidato número uno para explicar el
  11 % de pases completados**, por encima de la falta de percepción.
- **El balón no se roba: se recoge.** 22 robos contra 2015 recogidas cada 90
  minutos. Todo el mecanismo de entrada —enfriamientos, duelo, protección del
  cuerpo— decide el 1 % de los cambios de posesión. Por eso `steal_cooldown` no
  mueve ninguna métrica agregada: gobierna una centésima parte del partido.

### 4. Tests de propiedad causal ← el trabajo activo

Afirmar **dirección de efecto sobre N corridas**, no valores exactos. Son
inmunes al caos que nos mordió con el reloj: una propiedad afirmada sobre cien
corridas sobrevive a una perturbación de 1 ms; un marcador 1-0 no.

Y son inmunes a lo otro: "subir el umbral de tiro reduce los tiros" es
verificable con 51 goles por partido o con 2,7. Por eso este paso se puede
hacer hoy y la calibración no.

Hecho el 2026-07-30 en `crates/simulation/tests/causal_properties.rs`, con la
herramienta del paso 2 (`with_tuning` sobre la misma situación y las mismas
semillas, dos configuraciones):

1. ✅ **Simetría**: dos equipos idénticos terminan parejos (13-8 sobre seis
   partidos, cuota 0,62). Si fallara habría un sesgo de lado, y ninguna otra
   medida lo diría porque todas suman los dos equipos.
2. ✅ **El umbral de tiro manda sobre los tiros**: subir `ideal_position_gate`
   de 0,10 a 0,45 baja los disparos de 69 a 33 por 90 minutos.
3. ✅ **Caracterización — el alcance del receptor está muerto**: triplicarlo no
   cambia nada. Afirmado como está para que el día que se arregle, el test
   falle y haya que venir a convertirlo en la propiedad que quería ser.
4. ✅ **Caracterización — el balón se recoge, no se roba**: los robos son el
   1,4 % de los cambios de posesión.

Dos de las cuatro salieron al revés de lo que se buscaba, y ese fue el
rendimiento del paso: **intentar afirmar una propiedad es cómo se descubre que
una perilla no gobierna nada**. Un test que pasa no habría enseñado eso.

Las tácticas del norte (línea defensiva, presión alta) necesitan que el plan
sea configurable **por equipo**, y hoy `MatchTuning` es global: llegan con
MVP 5.

**Criterio de terminado:** ✅ cada desvío conocido con causa identificada y MVP
asignado, y ✅ cuatro propiedades afirmadas como test. **MVP 1.75 cerrado.**

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
- **El ritmo de gol es irreal**: 23/90 min contra ~2,7 reales (eran 51 antes de
  arreglar el fuera de juego, `REVISION_2026-07-30-reloj.md`). Ningún resultado
  de este simulador puede presentarse como predicción hasta calibrarlo.
- **El local gana siempre**: la simetría es un test rojo a propósito, con la
  causa localizada en la reanudación (arriba).
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
