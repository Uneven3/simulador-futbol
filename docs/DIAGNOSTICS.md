# Diagnóstico y logs

Cómo se observa una simulación que no se puede mirar. Implementado en MVP 1.5;
el diseño está tomado de `breath-of-freedom` (`src/debug/`, `src/perf/`), que ya
pagó los errores que aquí se evitan.

## Por qué existe

Antes el diagnóstico estaba en tres formas que no se hablaban entre sí:
`info!` sueltos dentro de los sistemas del kernel, campos de diagnóstico dentro
del estado autoritativo (incluido un `Vec<String>` en `MatchState`) y un forense
a mano dentro del test `long_match_stats`, que reimplementaba su propia
recolección. Ninguna de las tres se podía apagar, correlacionar ni comparar
entre corridas.

## Principios

1. **Un snapshot, dos sinks.** Los productores llenan datos puros; el HUD los
   dibuja y la consola los escribe. Ninguno formatea por su cuenta. Es la única
   forma de garantizar que pantalla y log no se contradigan.
2. **Snapshot y trace son cosas distintas.** El snapshot es el presente. El
   trace es un flujo de hechos por tick: describen momentos, y un snapshot sólo
   conservaría el último.
3. **Los hechos son valores tipados, no texto.** Una línea hay que parsearla
   para contarla. `MatchFact` es `Copy` y sin alocaciones: un test cuenta, un
   sink formatea, y añadir un consumidor no cuesta nada.
4. **Todo apagado por defecto.** Un log siempre encendido es un log que nadie
   lee. Cada canal se enciende cuando se está haciendo *esa* pregunta.
5. **Cada canal declara lo que cuesta** (`DiagnosticChannel::cost`), para que un
   interruptor barato se distinga del que distorsiona lo que mide.
6. **El tick es la clave de correlación.** Toda línea lleva el tick fijo en que
   ocurrió, o dos canales encendidos a la vez no se pueden cruzar.
7. **El diagnóstico no muta el estado autoritativo.** Sólo lee. Lo único que
   escribe son sus propios interruptores y su propio stream.
8. **Headless primero.** El trace y el snapshot viven donde funcionan sin
   ventana: son la forma de *ver* dentro de un test. El HUD es un sink más.

## Dónde vive cada parte

| Parte | Crate | Por qué |
|---|---|---|
| `MatchFact`, `DiagnosticChannel(s)` | `football_domain` (`diagnostics::facts`) | Un hecho es vocabulario; el kernel lo reporta y presentation lo lista |
| `MatchSnapshot`, `Field`, `SectionId` | `football_domain` (`diagnostics::snapshot`) | Si viviera en simulation, el HUD no podría leerlo sin romper la ley 1 |
| `MatchTelemetry` — stream por tick | `football_simulation` | Es telemetría del kernel; los tests headless la necesitan |
| `MatchLedger` — lo que los hechos suman | `football_simulation` | Deriva turnovers y rachas correlacionando el stream |
| `collect_snapshot` — llenar el presente | `football_simulation` | Lee estado autoritativo |
| Sink de consola | `football_simulation` | Debe funcionar sin ventana |
| Sink de HUD y overlays | `football_presentation` | Dibujo |
| Hub de canales (F1) | `football_presentation` | Entrada del usuario |

## Orden dentro del tick

`DiagnosticSet`, después de `SimulationSet::Referee`:

1. **Accumulate** — el ledger absorbe los hechos del tick y añade los que se
   derivan de ellos (un turnover se deduce correlacionando la suelta con quién
   acabó teniendo la pelota).
2. **Collect** — el presente se vuelca en el snapshot.
3. **Report** — la consola escribe lo de los canales encendidos.
4. **Close** — se cierra el tick. Nada puede leer el stream después.

## Canales

| Canal | Pregunta | Productores hoy |
|---|---|---|
| `Possession` | ¿quién tiene la pelota y por qué cambió de manos? | sí |
| `RefereeDecisions` | ¿qué vio el árbitro y qué decidió? | sí |
| `Touches` | ¿quién tocó la pelota, cuándo y con qué resultado? | sí |
| `PassOutcomes` | ¿los pases llegan, y si no, dónde se pierden? | sí |
| `PhaseTransitions` | ¿cuándo cambió el estado del partido? | sí |
| `Formation` | ¿el bloque mantiene forma o todos persiguen la pelota? | sólo carreras de ataque |
| `Performance` | ¿cuánto cuesta un tick? | ninguno todavía |

## Cómo se usa

**Con ventana:** `F1` abre el hub, `↑`/`↓` mueven, `Espacio` alterna, `P`
vuelca el snapshot al log. Overlays y canales son la misma lista.

**Headless:** `MatchKernelPlugin::new(scenario).retaining_facts(4096)` guarda
los últimos hechos para examinarlos al final. `render_pitch(world)` dibuja el
campo en ASCII desde cualquier punto de una corrida. `MatchLedger` tiene los
acumulados; `MatchTelemetry::recorded_on(canal)` filtra el stream.

`long_match_stats` es el ejemplo vivo de las tres cosas.

## Criterio de terminado

- ✅ ningún `info!` de gameplay suelto en un sistema del kernel;
- ✅ `MatchState` sin campos de diagnóstico;
- ✅ `long_match_stats` consume el subsistema en vez de reimplementarlo;
- ✅ todos los canales apagados por defecto y encendibles desde un hub;
- ✅ una corrida headless emite el mismo forense que antes se obtenía a mano.

## Pendiente

- `Performance` sin productor: falta el coste por tick, que es lo que dirá si
  MVP 6 puede correr muchas variantes de una situación.
- `Formation` casi sin productor: la forma del bloque (dispersión, altura de
  línea) es un hecho por derecho propio y hoy sólo se ve en el ASCII.
- El sink de consola escribe cada hecho en cuanto ocurre; falta el modo
  periódico estrangulado que `breath-of-freedom` usa para series temporales.
