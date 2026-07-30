# Diagnóstico y logs

Cómo se observa una simulación que no se puede mirar. Contrato pendiente de
implementar en MVP 1.5; el diseño está tomado de `breath-of-freedom`
(`src/debug/`, `src/perf/`), que ya pagó los errores que aquí se evitan.

## Por qué

Hoy el diagnóstico está disperso en tres formas que no se hablan entre sí:

- `info!`/`debug!` sueltos dentro de los sistemas del kernel;
- campos de diagnóstico **dentro del estado autoritativo** (`MatchState`
  guarda `turnovers_by_kind`, `pass_turnovers_near/far` y un
  `pass_turnover_log: Vec<String>`);
- forense a mano dentro del test `long_match_stats`, que reimplementa su propia
  recolección y sus propios snapshots ASCII.

Ninguna de las tres se puede apagar, correlacionar ni comparar entre corridas.

## Principios

1. **Un snapshot, dos sinks.** Los productores llenan una estructura de datos
   puros; el HUD la dibuja y la consola la escribe. Ninguno formatea por su
   cuenta. Es la única forma de garantizar que pantalla y log no se contradigan.
2. **Snapshot y trace son cosas distintas.** El snapshot es el presente
   (marcador, posesión, fase). El trace es un flujo de eventos por tick
   (toques, cambios de posesión, decisiones arbitrales, transiciones de fase):
   describen momentos, y un snapshot solo conservaría el último.
3. **Todo apagado por defecto.** Un log siempre encendido es un log que nadie
   lee. Cada canal se enciende cuando se está haciendo *esa* pregunta.
4. **Cada canal declara lo que cuesta.** Un colector caro se estrangula (p. ej.
   4 Hz) para que la herramienta de medición no aparezca en lo que mide.
5. **El tick es la clave de correlación.** Toda línea lleva el tick fijo en que
   ocurrió, o dos canales encendidos a la vez no se pueden cruzar.
6. **El diagnóstico no muta el estado autoritativo.** Solo lee. Lo único que
   escribe son sus propios interruptores.
7. **Headless primero.** El trace y el snapshot viven en la simulación y
   funcionan sin ventana: son la forma de *ver* en un test. El HUD es un sink
   más, no la fuente.

## Dónde vive cada parte

| Parte | Crate | Por qué |
|---|---|---|
| `MatchTelemetry`: stream de hechos por tick | `football_simulation` | Es telemetría del kernel; los tests headless la necesitan |
| `MatchSnapshot`: datos puros del presente | `football_simulation` | Un productor, varios consumidores |
| Sink de consola | `football_simulation` | Debe funcionar sin ventana |
| Sink de HUD y overlays | `football_presentation` | Dibujo |
| Hub de canales (teclas/panel) | `football_presentation` | Entrada del usuario |

El `MatchSnapshot` reemplaza los campos de diagnóstico que hoy contaminan
`MatchState`, y el trace reemplaza los `info!` sueltos.

## Canales previstos

Cada uno responde una pregunta concreta:

| Canal | Pregunta |
|---|---|
| `Possession` | ¿quién tiene la pelota y por qué cambió de manos? |
| `RefereeDecisions` | ¿qué vio el árbitro y qué decidió? |
| `Touches` | ¿quién tocó la pelota, cuándo y con qué resultado? |
| `PassOutcomes` | ¿los pases llegan, y si no, dónde se pierden? |
| `PhaseTransitions` | ¿cuándo cambió el estado del partido? |
| `Formation` | ¿el bloque mantiene forma o todos persiguen la pelota? |
| `Performance` | ¿cuánto cuesta un tick? |

## Snapshots de campo

El snapshot ASCII del campo (hoy dentro del test `long_match_stats`) es la
forma más barata de *ver* la forma táctica sin ventana, y por eso pertenece al
subsistema de diagnóstico y no a un test: cualquier corrida headless debe poder
pedirlo.

## Criterio de terminado

- ningún `info!` de gameplay suelto en un sistema del kernel;
- `MatchState` sin campos de diagnóstico;
- `long_match_stats` consume el subsistema en vez de reimplementarlo;
- todos los canales apagados por defecto y encendibles desde un hub;
- una corrida headless puede emitir el mismo forense que hoy se obtiene a mano.
