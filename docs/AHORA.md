# Ahora

## Lo primero de la próxima sesión

**Crear el repositorio en GitHub** (acordado con el usuario el 2026-07-30). El
proyecto vive solo en local: nueve commits sin remoto. Todo lo escrito en
`docs/` está pensado para que alguien lo retome, y eso hoy no se puede.

Después de eso, el objetivo activo.

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

### 1. Las envolventes son dato, no literales

Hoy las decisiones que fijan el resultado viven como números sueltos dentro de
`player_decisions.rs`: el umbral de tiro, el `possession_amount > 0.99`, los
24 m de amenaza, los cooldowns de toque y de robo. No se pueden barrer sin
editar código, y nadie sabe cuáles son sin leerlo.

Extraerlas a un `MatchTuning` versionado, como ya lo es `MatchRegulations`, con
**un solo lugar** donde vive cada valor por defecto. Sin esto no hay nada que
girar, y los tres pasos siguientes no se pueden hacer.

### 2. La envolvente, como herramienta

`seeded_envelope` es hoy un test `#[ignore]` con diez semillas y cuatro
métricas. Convertirlo en algo que corra N partidos y reporte **distribuciones**:
histograma de goles por partido, marcadores, tiros, tiros a puerta, posesión.

El histograma es lo importante: la referencia real son ~1,35 goles por equipo,
casi una Poisson, y comparar histogramas dice mucho más que comparar medias.

### 3. Calibrar contra la distribución, no contra la media

Sospechosos por orden: el portero no defiende de verdad, el umbral de tiro deja
disparar desde cualquier sitio, y no hay faltas que interrumpan.

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
