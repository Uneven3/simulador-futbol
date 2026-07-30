# MVP 1.5 — qué se hizo y por qué

Consolidación: no añade capacidades, paga lo que hoy es barato y en cada MVP
que pase deja de serlo. El plan salió de `REVISION_2026-07-30.md`; esto es lo
que quedó hecho, en el orden en que se hizo.

| Commit | Qué |
|---|---|
| `87898a0` | Identidad de dominio: personas y equipos en vez de `Entity` |
| `69543f3` | Diagnóstico: un stream de hechos, un snapshot, dos sinks |
| `3eb4700` | Nombres del dominio actual, y la higiene pendiente del port |
| (este) | El tiempo del kernel como `Duration` — y lo que eso destapó |

## 1. Identidad (ley 10)

`Entity` es un hueco en un mundo: no sobrevive a un despawn, no se serializa y
no dice nada en un log. Se usaba como memoria persistente en ocho sitios, así
que una sustitución (MVP 2), un escenario guardado o un replay habrían
entregado la pelota a un extraño.

- `PlayerId { team, shirt }` — el fútbol ya nombra así a la gente, y un log dice
  `Away #9` sin tabla de traducción.
- `TeamId` con `opponent()`, y `ByTeam<T>` en lugar de `[T; 2]` indexado con
  `team as usize`. El `1 - team` para llegar al rival era silencioso al fallar.
- `PlayerRegistry` traduce identidad → cuerpo. Se mantiene desde el mundo
  (`Added<Player>` / `RemovedComponents`), así que un cuerpo que se va deja de
  resolver en vez de dejar un id colgando.
- `PlayerReading` (antes `PlayerSnap`) ya no lleva `Entity`: nada de lo que las
  decisiones recuerdan entre ticks está atado al almacenamiento.

**Decisión que puede sorprender:** los dorsales desambiguan las posiciones
repetidas. `normalized_formation_position` distingue a los dos centrales por el
3 y el 4, no por un índice — que es como lo hace una alineación.

## 2. `Player` separado en cuatro

Envejecen distinto, y juntos obligaban a pedir acceso de escritura al jugador
entero para leer un dorsal.

| Componente | Qué es | Quién lo llena |
|---|---|---|
| `Player` | identidad e instrucción: id, posición, rol, hueco en la formación | antes del pitazo |
| `Attributes` | lo que puede hacer: velocidad punta, altura, técnica de tiro | MVP 3 lo amplía y lo calibra |
| `Mentality` | lo que está dispuesto a hacer: work rate | MVP 5 lo amplía |
| `PlayerMatchState` | lo que el partido le hizo: velocidad reciente, último toque, marca | cada tick |

`PlayerRole` se partió en `PlayingPosition` (dónde se alinea) y `TacticalRole`
(qué se le pide), como pide `ARCHITECTURE.md`. La equivalencia del port se
conserva como `PlayingPosition::default_role()`, con un test que verifica que
los sesgos de ataque no se movieron — y ahora un lateral puede recibir un rol
que contradiga su posición, que es el único motivo por el que separar sirve.

**Regla de admisión aplicada:** `stamina`, `acceleration` y `agility` no las
leía nadie. Salieron. Vuelven en MVP 3 con el mecanismo que las use.

## 3. Diagnóstico y logs

Diseño tomado de `breath-of-freedom` (`src/debug/`), que ya pagó estos errores,
con una diferencia deliberada: **los hechos son valores tipados, no líneas de
texto.** Una línea hay que parsearla para contarla, y por eso el test forense se
había construido su propia contabilidad paralela.

- `MatchFact` (en `domain`) — lo que el kernel reporta. `Copy`, sin alocaciones.
- `MatchTelemetry` — el stream por tick. El tick es la clave de correlación: sin
  él, dos canales encendidos a la vez no se pueden cruzar. Retención opcional
  (`retaining_facts`), porque un partido en vivo no tiene por qué guardar nada.
- `MatchLedger` — lo que los hechos suman. Deriva los turnovers correlacionando
  la suelta con quién acabó teniendo la pelota, así el kernel no lleva contadores
  y "recuperar tu propio pase" deja de contar como pérdida (el contador del port
  no sabía distinguirlos).
- `MatchSnapshot` — el presente como datos puros. **Vive en `domain`**, no en
  simulation, porque si no el HUD no podría leerlo sin que presentation dependa
  del kernel.
- `render_pitch` — el campo en ASCII, que estaba dentro de un test.
- Hub F1 (`crates/presentation/src/debug_hub.rs`): overlays y canales son una
  sola lista de datos, con ↑/↓, Espacio y P. Cinco teclas F sin jerarquía era
  historia acumulada, no un diseño.

Todo apagado por defecto. `MatchState` no tiene ni un campo de diagnóstico, y
`long_match_stats` consume el subsistema en vez de duplicarlo.

**Lo que reveló al conectarlo:** de 174 pases perdidos, 152 se pierden en camino
y sólo 22 en la recepción; y de 231 cambios de posesión, 225 son balón suelto
contra 6 entradas. Eso ya estaba pasando, pero no se podía consultar.

## 4. Nombres (ley 16)

`eliza.rs` → `player_decisions.rs`, `team_ai.rs` → `team_tactics.rs`,
`PlayerSnap` → `PlayerReading`, `ElizaCtx` → `DecisionContext`, `TeamAi` →
`TeamShape`, `mind_set` → `attacking_bias`.

**Lo que a propósito no se tocó:** las referencias al original en los
comentarios. Dicen de dónde viene un algoritmo, apuntan a
`references/gameplay_football/` y las envolventes siguen sin calibrarse contra
otra cosa; borrarlas quitaría trazabilidad a cambio de nada. Lo que sí se
corrigió son los comentarios que describían el presente con nombres del pasado.

## 5. Higiene

- **Clippy limpio con `-D warnings`.** De los 19 lints heredados, los mecánicos
  los arregló `clippy --fix`; el resto eran queries de Bevy sin nombre (ahora
  `BallSystemBody`, `DecidingPlayer`) y funciones con parámetros que viajaban
  juntos sin decirlo (`Toucher`, `AdaptedFor`). Los dos `allow` que quedan son
  sistemas de Bevy, donde los parámetros son las dependencias declaradas.
- **`Scenario::contradictions()`** — un escenario que pide un gol y que el juego
  nunca se detenga, o que corre 90 minutos dentro de una suite, se rechaza antes
  del primer tick. `assert_scenario_holds` lo comprueba.
- **Escenario de red lateral** — medio metro por fuera del poste no es gol.
- **Fin de partido como estado real** — en `FullTime` los sistemas de decisión y
  de toque dejan de correr y las velocidades se ponen a cero. La pelota sí sigue
  integrándose: rueda hasta pararse, como cuando el pitazo la agarra en el aire.
  Lo que se acaba es el fútbol, no la física.

## 6. Unidades de tiempo (ley 9), y el hallazgo

El kernel medía el tiempo con `(time.elapsed_secs_f64() * 1000.0) as u64` en
cada sistema. Ahora todo es `Duration`: `MatchState.period_elapsed`,
`restart_in`, `possession_since`, `Ball.last_touch_at`, y los cooldowns como
`Duration::from_millis(220)` en vez de `> 220`.

Lo que apareció al hacerlo está en `REVISION_2026-07-30-reloj.md` y merece
leerse entero. En corto: **el contador viejo perdía un milisegundo en el 1 % de
los ticks**, y arreglarlo duplicó el ritmo de gol. Eso no significa que la
conversión rompiera nada — significa que el modelo estaba apoyado en el ruido
de un reloj mal convertido, y que sus envolventes nunca se compararon con
ninguna referencia: **51 goles cada 90 minutos contra ~2,7 reales**.

De ahí sale la otra corrección de este corte: la "métrica de no regresión" que
usábamos (1-0, 205 cambios de posesión) era una trayectoria, no una métrica.
La sustituye `seeded_envelope`, que corre diez semillas y reporta tasas.

## Verificación

- **62 tests en verde**, de 36 al empezar.
- **Cero deriva de gameplay en los tres primeros cortes**: identidad,
  diagnóstico y nombres dieron exactamente los mismos números (1-0, 21
  tocadores, 205 cambios, racha 16,7 s). Se midió después de cada uno.
- **El cuarto corte sí movió el juego, y con razón**: el reloj exacto cambia el
  ritmo de gol de 27 a 51 por 90 minutos. Verificado sobre diez semillas, no
  sobre una.
- `cargo clippy --all-targets -- -D warnings` sin salida.

**No verificado:** nada visual. Y ahora sabemos algo peor que "sin calibrar":
el modelo está a un orden de magnitud de la única referencia externa trivial de
conseguir.
