# Arquitectura

Este documento fija leyes; el detalle vive en módulos, tests y documentos de
dominio. El código las cita por § y no se mergea código que las viole.

## Capas

```text
football_app          → football_simulation → football_domain
football_presentation → football_simulation
```

- **Domain** (`crates/domain`): tipos, unidades, reglas, hechos, intents y
  configuración.
- **Simulation** (`crates/simulation`): ECS autoritativo, física, percepción,
  decisión, táctica, arbitraje y telemetría.
- **Presentation** (`crates/presentation`): visuales, cámara, animación, UI,
  audio y overlays; solo lee.
- **App** (paquete raíz `gameplayfootball`): composición, escenario y ciclo de
  vida.

Son crates, así que Cargo impide las dependencias inversas. Domain y simulation
dependen de subcrates de Bevy (`bevy_ecs`, `bevy_math`, `bevy_time`, `bevy_app`,
`bevy_log`) y **no** de `bevy`: sin `bevy_render`, `bevy_pbr` ni `bevy_asset` en
el grafo, una regla no puede expresarse en términos de un mesh. Solo
presentation y app ven el motor completo.

## Las quince leyes

Se dan por sabidas SOLID, las capas de la arquitectura limpia y que un nombre
bien puesto vale más que el comentario que lo explica. Lo que sigue es esa base
traducida a este proyecto: a ECS, donde no hay herencia y la responsabilidad
única se mide por sistema; y a Rust, donde una ley que el compilador puede
cobrar no se deja en manos de un revisor. Son quince porque una ley que no se
recuerda no se aplica, y ninguna está aquí si otra ya la dice.

### De dominio: qué es verdad en un partido y quién puede decidirlo

1. **Headless y sin autoridad visual.** Los tests de simulación no registran
   `Mesh`, `StandardMaterial`, `Image`, ventanas ni `AssetServer`; la entidad
   autoritativa no tiene visuales y otra la sigue con
   `VisualOf(simulation_entity)`. Ningún bone, clip ni geometría decide
   contacto, velocidad ni resultado: si un asset cambia el partido, el partido
   estaba en el asset.
2. **Los datos son datos, y cada estado tiene un escritor.** Components,
   Resources y Messages no traen comportamiento; quien no es dueño de un estado
   publica un intent o un hecho y espera. Dos escritores no dan un error: dan un
   resultado que depende del orden de los sistemas.
3. **Verdad, conocimiento, intención, ejecución y arbitraje son cinco cosas.**
   Una decisión no lee lo que su jugador no ha observado; el motor entrega la
   acción alcanzable, no la pedida; el árbitro juzga lo que vio, así que un
   incidente puede quedar sin pitar. Saltarse un escalón convierte el simulador
   en un guion: el resultado sale igual, pero ya no lo produjo nadie.
4. **Unidades explícitas e identidad propia.** Segundos, metros y m/s en las
   firmas, no un `f32` sin nombre. Los IDs de dominio son newtypes; `Entity` es
   transitorio y nadie lo guarda como memoria.
5. **Un resultado es reproducible y comparable.** Aleatoriedad inyectada y
   semilla registrada, reglas versionadas por edición IFAB y competición, y toda
   afirmación de fidelidad con métrica, referencia y tolerancia. Un número sin
   las tres no dice si el cambio mejoró algo.

### De ingeniería: cómo se escribe el código que lo sostiene

6. **Una responsabilidad por sistema, y el tamaño la delata.** Si el nombre
   necesita una "y", son dos sistemas; ~300 líneas por archivo y ~80 por función
   son señal de dividir, no un bloqueo. Un sistema que gana la posesión, resuelve
   la disputa y ejecuta el disparo no se calibra ni se testea por partes, y un
   archivo que crece sin dividirse deja de tener dueño.
7. **Se extiende agregando, no editando.** Una regla nueva entra como sistema o
   componente nuevo en su `SystemSet`, no como una rama más dentro de un sistema
   existente. Si añadir faltas obliga a editar el sistema del regate, la costura
   está en el sitio equivocado.
8. **El sistema lee, llama y escribe.** La lógica calculable vive en funciones
   puras, probables sin `App`; los parámetros que fijan el resultado son dato
   versionado (`MatchTuning`, `MatchRegulations`) con un solo hogar por valor por
   defecto; más de ocho parámetros se agrupan en `SystemParam`, y cada
   `#[allow(clippy::too_many_arguments)]` dice por qué sigue ahí. Un número
   dentro de la lógica no se puede barrer ni reportar junto al resultado que
   produjo.
9. **La frontera de un crate es su API, no la suma de sus módulos.** `pub` solo
   lo que otro crate consume de verdad. Una dependencia nueva lleva OK humano
   previo y el contrato que resuelve escrito en el `Cargo.toml` que la añade.
10. **Los estados imposibles no compilan.** Lo que tendría que vigilar un
    revisor, lo vigila el tipo: unidades como newtype de campo privado
    (`Seconds`, `Metres`) y no `f32` sueltos que se confunden entre sí; el dato
    que solo existe en una variante, dentro de esa variante y no en banderas
    paralelas; campos privados con un único mutador en el módulo dueño, que es
    §2 verificada por el compilador; y `match` sin `_ =>`, para que un enum
    nuevo rompa el build en todos sus usos. **Si una ley de este archivo se
    puede convertir en un error de compilación, se convierte:** la que necesita
    revisor es la que se incumple. Por eso los lints van en la sección `[lints]`
    de los cuatro `Cargo.toml` —no se heredan, porque el workspace es
    compartido— y no en la memoria de quien commitea.
11. **Rust seguro, y el idioma es el de Bevy.** Sin `unsafe`; `Option` y
    `Result` en vez de centinelas; APIs pequeñas. Cuando dos crates sugieren
    formas distintas de hacer algo, manda Bevy, y cuando Bevy cambia la suya en
    una versión nueva, migramos en vez de conservar la vieja: un idioma propio
    envejece contra el motor y lo paga cada upgrade. `cargo fmt` y
    `clippy -D warnings` antes de dar algo por terminado, con cada `#[allow]`
    justificado en su línea.
12. **Sin allocations por tick.** Los buffers se reutilizan tras medir su
    capacidad. Un `collect()` en el camino caliente no se nota en un test de
    escenario y sí en noventa minutos a 100 Hz.
13. **Nombres del dominio actual, comentarios de invariantes.** El nombre dice
    fútbol, no Gameplay Football. El comentario dice el invariante o la
    restricción, nunca el *qué*: eso lo dice el código, y si hacen falta más de
    tres líneas el arreglo es el nombre o partir la función, no escribir mejor
    el comentario. Única excepción: la procedencia del original, que es
    trazabilidad hacia `references/gameplay_football/`.
14. **Un mecanismo sin test no existe.** Cada regla entra con su escenario
    IFAB, cada función pura con sus casos de borde, y la fidelidad se demuestra
    con la envolvente de diez semillas, nunca con una corrida (`VALIDATION.md`).
    Un test que sigue pasando con la lógica invertida no estaba probando nada.
15. **Ni una medición, ni una fecha, ni el relato de una sesión.** Las
    mediciones van a `measurements/` y la historia al mensaje de commit; un
    documento que hay que reeditar cuando cambia una cifra es una copia de un
    CSV. La prosa del repositorio tiene techo de 1000 líneas y lo cobra
    `tests/documentation_budget.rs`, porque un techo escrito dentro de un
    documento no se aplica solo.

## Pipeline

El orden objetivo, en `SystemSet` semánticos y no en el heredado de
`Match::Process()`. `FixedUpdate`: ciclo del partido → percepción del mundo →
observaciones → creencias → responsabilidades tácticas → intenciones → plan
motor → cuerpos y contactos → integración del balón → incidentes físicos →
observación y decisión arbitral → transiciones y telemetría. `Update`:
interpolación del snapshot → visuales → overlays, cámara, UI y audio.

## Mapa de módulos

Lo que no aparece aquí no tiene dueño, y eso es un bug de arquitectura antes que
de código.

| Módulo | Posee | Frontera |
|---|---|---|
| `domain::identity` | `PlayerId`, `TeamId`, `ByTeam`, `PlayerRegistry` | Única traducción identidad ↔ `Entity`; nadie más guarda `Entity` como memoria |
| `domain::match_state` | `MatchState`, `MatchRegulations`, `PitchConfig`, `MatchRng` | El estado del partido; solo el kernel lo escribe |
| `domain::tuning` | `MatchTuning` y sus grupos | Un único hogar por valor por defecto (§8) |
| `domain::scenario` | `Scenario`, `Expectations`, `ScenarioOutcome` | La situación reproducible completa: estado inicial, semilla, ventana y afirmaciones |
| `domain::player` | `Player`, `Attributes`, `Mentality`, `PlayerMatchState` | Identidad e instrucción, capacidad, disposición y lo que el partido escribe, separados |
| `domain::math` | Geometría y RNG puros | Sin Bevy más allá de `bevy_math`; todo testeable sin `App` |
| `simulation::match_setup` | Instalación del escenario y de los cuerpos | Único que hace spawn de entidades autoritativas |
| `simulation::match_clock` | Reloj y fases (Ley 7 IFAB) | Único escritor de `period_elapsed` y `phase` |
| `simulation::team_tactics` | `TeamTactics`, forma del bloque, trampa del fuera de juego | Lee estado, escribe solo su recurso |
| `simulation::perception` | Qué ve cada jugador y qué recuerda | Único escritor de `ObservationMemory` y `Beliefs`; la decisión lee de aquí y no del mundo |
| `simulation::player_decisions` | Adónde corre cada jugador y qué hace con el balón | Decide; no ejecuta ni toca el balón. Escribe `MovementIntent` y nunca `Velocity` |
| `simulation::locomotion` | Lo que un cuerpo consigue de lo que le pidieron, y lo que le queda de piernas | Único que convierte intención en `Velocity` y único escritor de `FatigueState` (§3) |
| `simulation::player_movement` | Designación de posesión, integración de cuerpos y separación | Único escritor de `Position` de los jugadores |
| `simulation::ball_contest` | De quién es el balón: escapadas, contacto, entrada, recogida | Único que otorga la posesión; publica `BallContest` como hecho del tick |
| `simulation::force_field` | `ForceSpot`, `Falloff`, la suma de atractores | Dónde quiere estar un cuerpo; no lo mueve |
| `simulation::goalkeeping` | Qué pasa cuando el balón llega a la portería | Toca el balón; la posesión sigue siendo del contest |
| `simulation::ball_release` | Cómo sale el balón del pie: tiro, pase, despeje, conducción, y el `ActionCommitment` que los arma | Ejecuta la decisión ajena; las recetas son funciones puras |
| `simulation::ball_physics` | Integración y predicción del balón | La predicción es la trayectoria futura real; nadie más la calcula |
| `simulation::ball_collisions` | Contacto balón-cuerpo y balón-portería | Emite hechos; no decide reglas |
| `simulation::referee` | Fuera de juego, fuera de banda, gol y reanudaciones | Único que otorga `SetPiece` y cambia el marcador |
| `simulation::diagnostics` | `MatchFact`, `MatchTelemetry`, `MatchLedger`, `MatchSnapshot` | Solo lee estado autoritativo; apagado por defecto |
| `presentation::*` | Visuales, cámara, HUD, overlays, hub de depuración | Solo lee; borrar el crate deja un partido completo |
| `src/` (app) | Composición, catálogo de escenarios, `ScenarioRunner` | Cablea capas; no decide nada de fútbol |

## Nomenclatura

- Sistemas en verbo (`integrate_ball_motion`); funciones puras por lo que
  devuelven (`estimate_interception_time`), nunca `get_` si se calcula o estima.
- Hechos en sustantivo (`BallContact`, `RestartAwarded`); solicitudes con sufijo
  (`KickIntent`, `SubstitutionRequest`). `Goalkeeper`, no `GK`.

## Rust y deuda

El borrow checker es la herramienta, no el obstáculo: contra un escritor cruzado
la respuesta son queries disjuntas, componentes descompuestos, fases de
lectura/propuesta/aplicación y mensajes —de eso mismo saca Bevy su paralelismo—.

**Qué viola hoy estas leyes** se mide, no se narra:
`wc -l crates/*/src/*.rs | sort -n` dice qué archivos pasan de las ~300 líneas
de §6, `grep -rn "too_many_arguments" crates/` los parámetros de §8, y
`grep -rn "_ =>" crates/` los enums que hoy no rompen el build (§10). Se
parten al tocarlos por otra razón; partirlos antes es riesgo sin lector. Y la
división se valida con la envolvente: si las diez semillas dan los mismos
números, el refactor fue fiel (`VALIDATION.md`).
