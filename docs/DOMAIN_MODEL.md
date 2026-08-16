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

**Cuerpos.** `Position` es la única verdad espacial —metros, Z arriba— y la
acompañan `Velocity` y `Facing`, que es el cuerpo y no la vista: de él cuelga a
qué ritmo se corre. Un `Transform` nunca es autoritativo. Un cuerpo **no es una
partícula**: lo que puede hacer depende de hacia dónde, porque no lo limita lo
mismo. Empujar adelante lo limita la pierna (`acceleration`); frenar y cortar
los limita el agarre (`grip`), que da dos g largos. De ese reparto salen solos
el corte en seco, la curva al girar a la carrera y la carrera perdida al
cambiar de dirección.
`MovementIntent` es la velocidad que se pidió y `Velocity` la que se consiguió:
entre las dos está el motor, `Attributes` dice cuánto separa a una de la otra y
`FatigueState` cuánto de eso queda a estas alturas del partido.
`Player` es identidad e instrucción, `Attributes` capacidad, `Mentality`
disposición y `PlayerMatchState` lo que el partido le va escribiendo.
`PlayingPosition` es la posición nominal y `TacticalRole` la función.

**Balón.** `Ball` lleva su momentum y su predicción, que hoy es la trayectoria
futura real y todo el mundo lee: es la omnisciencia que queda por romper.

**Percepción.** `Vision` es el cono, cuelga de `Looking` —los ojos, no el
pecho— y `hidden_by` le pone la línea: un cuerpo delante esconde del todo o deja
ver un trozo, que es situar peor. Lo que la vista no alcanza lo alcanza el
oído: un compañero a `shout_range` canta lo que sabe del balón, sin cono ni
línea y con `SHOUTED_BLUR` encima. `Observation` es lo último que se supo de
otro, cuándo y con cuánta duda (`blur` lo que se falló al verlo, `uncertainty`
lo escapado desde entonces); entra en `ObservationMemory` `reaction` después y
con la hora en que se vio, y `Beliefs` arma el campo que se lee en vez de la
verdad. La
duda viaja en `PlayerReading` y decide: se barre buscando al peor situado, se va
a por el balón si se duda más de `lost_ball_doubt`, un pase a quien no se tiene
situado vale menos, y se persigue la idea del balón hasta `eyes_on_the_ball`.
`Judgement` sesga lo visto y `Senses` define cono, reacción, barrido, oído y
cuello; ambos salen de la semilla y se espejan por dorsal. `TacticalPlan` es la
política por equipo; `TacticalResponsibility`, la ocupación, cobertura, presión
o apoyo que consume cada decisión. Las familiaridades limitan cobertura, no dan
un bonus global.

**Hechos y decisiones.** `BallTouched` y `PotentialFoul` son hechos: ocurrieron,
y no dicen qué hacer con ellos. `SetPiece` y `OffsideRecords` ya son decisión del
árbitro. Que un hecho quede sin decisión es la ley 3, no un descuido.

**Parámetros.** `MatchTuning` agrupa lo que fija el resultado, por mecanismo
(`ContestTuning`, `PassingTuning`, `GoalkeepingTuning`, `RefereeTuning`…), con
su `TuningVersion`: un número que decide algo vive ahí y en ningún otro sitio.

**Situaciones.** `Scenario` es una situación reproducible completa —estado
inicial, semilla, ventana y afirmaciones—, `Expectations` lo que debe pasar y
`ScenarioOutcome` lo que pasó. `PlayerPlacement` reconstruye una posición,
rol y casilla individual; `MovementProposal` propone una intención y deja que
el motor resuelva el cuerpo que realmente resulta. `InitialObservation` declara
la creencia con la que un observador empieza, no una lectura inicial de la
verdad. El RON versionado intercambia escenario, reglamento, tuning, planes,
balón, alternativas y esas creencias. `CounterfactualReport` corre cada propuesta
sobre las mismas semillas y conserva su envolvente, sin
elegir una trayectoria como si fuera la única consecuencia.

## Vocabulario reservado

Nombres que aún no existen, con el MVP que los trae, para que el día que
aparezcan no se inventen dos veces:

- **Percepción (MVP 4):** `Ball.predictions` sigue siendo la trayectoria real;
  cada jugador conserva su propia extrapolación en `Beliefs`. Cómo funciona ver
  no es de nadie y no será atributo.
- **Motor (MVP 3):** `BodyEnvelope`, y un `ActionCommitment` con fases en vez de
  un solo instante de contacto.
- **Arbitraje:** `RefereeObservation`, que separa lo que el árbitro vio de lo
  que ocurrió. Hoy ve todo lo que se publica.
- **Unidades:** `Metres`, `Seconds`, `MetresPerSecond` como newtypes de campo
  privado (§10). Hoy son `f32` en las firmas, que es la deuda que §4 declara.

## Representación

`VisualOf(Entity)` enlaza una representación con su entidad autoritativa.
Primitiva, low-poly, repetición y visual remoto son consumidores equivalentes, y
ningún tipo de dominio lleva rutas de assets.
