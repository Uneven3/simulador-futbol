# Modelo de dominio

El vocabulario del partido, en dos mitades a propósito: lo que existe se busca
en el código con estos nombres, y lo reservado es la forma que tendrá cuando
llegue su MVP. Un nombre de la primera mitad que no compile es un error de este
documento.

## Lo que existe

**Identidad.** `TeamId` es identidad y no geografía: qué mitad defiende un
equipo es `TeamSide`, y quién defiende cuál ahora mismo es `PitchSides`, que
cambia una vez, en el descanso. `PlayerId` es equipo más dorsal y sobrevive a
escribirse en un log; `Entity` no, y por eso nadie lo guarda como memoria.
`PlayerRegistry` es la única traducción entre ambos.

**Partido.** `MatchState` es lo que un marcador no cuenta: fase (`MatchPhase`),
reanudación pendiente (`SetPiece`), posesión, y el tiempo del periodo con lo que
se pasó parado. `MatchRegulations` son las duraciones de la competición,
`PitchConfig` las medidas del campo y `MatchRng` la semilla.

**Cuerpos.** `Position` es la única verdad espacial —metros, Z arriba— y
`Facing` y `Velocity` la acompañan; un `Transform` nunca es autoritativo. Un
cuerpo **no es una partícula**: lo que puede hacer depende de hacia dónde,
porque no lo limita lo mismo. Empujar hacia adelante lo limita la pierna
(`acceleration`); frenar y cortar los limita el agarre (`grip`), que da dos g
largos. De ese reparto salen solos el corte en seco, la curva al girar a la
carrera y la carrera que se pierde al cambiar de dirección.
`MovementIntent` es la velocidad que se pidió y `Velocity` la que se consiguió:
entre las dos está el motor, `Attributes` dice cuánto separa a una de la otra y
`FatigueState` cuánto de eso queda a estas alturas del partido.
`Player` es identidad e instrucción, `Attributes` capacidad, `Mentality`
disposición y `PlayerMatchState` lo que el partido le va escribiendo.
`PlayingPosition` es la posición nominal y `TacticalRole` la función.

**Balón.** `Ball` lleva su momentum y su predicción, que hoy es la trayectoria
futura real y todo el mundo lee: es la omnisciencia que queda por romper.

**Percepción.** `Vision` es el cono —ángulo, detalle y alcance— y `blocks_the_view`
la línea: un cuerpo delante esconde lo que hay detrás, y la sombra se ensancha
con la distancia. `Observation` es lo último que se supo de un cuerpo, cuándo y
con cuánta duda: `blur` es lo que se falló al verlo y `uncertainty` lo que se
escapó desde entonces —el error solo se calcula desde fuera, comparando con la
verdad; la duda el jugador sí la sabe—. Lo visto entra en `ObservationMemory`
`reaction` después y con la hora en que se vio, no con la de ahora, y `Beliefs`
arma con todo eso el campo que la decisión lee en vez de la verdad.

`Gaze` es dónde se quiere mirar: la atención barre fuera del balón con la
cadencia de `PerceptionTuning`, y en el barrido busca al cuerpo peor situado del
hombro que toca. Si la duda sobre el balón pasa de `lost_ball_doubt`, deja de
reconocer y va a buscarlo. La duda viaja en `PlayerReading` y ya cambia
decisiones: un pase a quien no se tiene situado vale menos, y lo que se persigue
es la idea del balón hasta `eyes_on_the_ball`, donde manda el pie.

**Hechos y decisiones.** `BallTouched` y `PotentialFoul` son hechos: ocurrieron,
y no dicen qué hacer con ellos. `SetPiece` y `OffsideRecords` ya son decisión del
árbitro. Que un hecho quede sin decisión es la ley 3, no un descuido.

**Parámetros.** `MatchTuning` agrupa lo que fija el resultado, por mecanismo
(`ContestTuning`, `PassingTuning`, `ShootingTuning`, `GoalkeepingTuning`,
`RefereeTuning`…), con su `TuningVersion`. Un número que decide algo vive ahí y
en ningún otro sitio.

**Situaciones.** `Scenario` es una situación reproducible completa —estado
inicial, semilla, ventana y afirmaciones—, `Expectations` lo que debe pasar (no
todo lo que puede) y `ScenarioOutcome` lo que pasó.

## Vocabulario reservado

Nombres que aún no existen, con el MVP que los trae. Están escritos para que el
día que aparezcan no se inventen dos veces:

- **Percepción (MVP 4):** `Ball.predictions` sigue siendo la trayectoria futura
  real, y quien la persigue la persigue desviada por lo que cree. Falta que cada
  uno prediga la suya, y que taparse sea situar peor y no dejar de ver.
- **Motor (MVP 3):** `BodyEnvelope`, y un `ActionCommitment` con fases en vez
  de un solo instante de contacto. La aceleración, el frenado y la fatiga ya no
  están aquí: existen, con el mecanismo que los lee.
- **Táctica (MVP 5):** `TacticalPlan`, `TacticalResponsibility`,
  `PositionFamiliarity`, `RoleFamiliarity`. Un atributo modifica operaciones
  concretas; nunca es un bonus global.
- **Arbitraje:** `RefereeObservation`, que separa lo que el árbitro vio de lo
  que ocurrió. Hoy ve todo lo que se publica.
- **Unidades:** `Metres`, `Seconds`, `MetresPerSecond` como newtypes de campo
  privado (§10). Hoy son `f32` en las firmas, que es la deuda que §4 declara.

## Representación

`VisualOf(Entity)` enlaza una representación con su entidad autoritativa.
Primitiva, low-poly, repetición y visual remoto son consumidores equivalentes, y
ningún tipo de dominio contiene rutas de assets.
