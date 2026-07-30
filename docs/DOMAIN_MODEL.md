# Modelo de dominio

## Entidades

- **Match:** reglas, competición, equipos, escenario, reloj y semilla.
- **Team:** identidad/miembros; el plan táctico es `TacticalPlan`.
- **Player:** participante estable identificado por `PlayerId`; `Entity` puede
  cambiar durante el partido.
- **Ball:** cuerpo físico. No emite conocimiento ni predicciones; cada jugador
  estima a partir de percepción.
- **MatchOfficial:** participante situado con percepción y autoridad.

## Perfil y estado

Persistente:

- `PlayerIdentity`, `BodyProfile`, `TechnicalProfile`, `CognitiveProfile`;
- `PositionFamiliarity`, `RoleFamiliarity`.

Situado:

- `BodyState`, `FatigueState`;
- `PlayingPosition`, `TacticalRole`, `TacticalResponsibility`;
- `ObservationMemory`, `BeliefState`;
- `PlayerIntent`, `ActionCommitment`.

Un atributo modifica operaciones concretas. `PositionFamiliarity`, por ejemplo,
puede afectar reconocimiento e incertidumbre; no es un bonus global.

## Espacio y cuerpo

Tipos autoritativos: `WorldPosition`, `FacingDirection`, `LinearVelocity`,
`AngularVelocity`, `BodyEnvelope`. `Transform` es representación del engine, no
vocabulario público.

El cuerpo usa envolventes/capacidades independientes del mesh. Bones representan
una acción, pero no son la fuente única de verdad física.

## Verdad y conocimiento

- `WorldTruth`: estado autoritativo.
- `Observation`: medición recibida con precisión y timestamp.
- `ObservationMemory`: historial acotado propio.
- `BeliefState`: estimación, incertidumbre y edad.

Decisiones leen `BeliefState`, no `WorldTruth`, salvo tests explícitamente
perfectos.

## Táctica y ejecución

- `PlayingPosition`: posición nominal.
- `TacticalRole`: función contextual dentro del plan.
- `TacticalResponsibility`: zona, rival, carril o apoyo activo.
- `PlayerIntent`: objetivo todavía sin garantía física.
- `ActionCommitment`: plan motor con duración, fases y cancelación.

## Reglas y arbitraje

- `PhysicalFact`: hecho sin juicio.
- `PotentialIncident`: hechos que podrían requerir decisión.
- `RefereeObservation`: qué observó un oficial.
- `RefereeDecision`: aplicación de regla.
- `MatchTransition`: cambio de fase, reloj, sanción, marcador o restart.

## IDs y unidades

IDs son newtypes (`MatchId`, `TeamId`, `PlayerId`, `CompetitionId`), no índices
desnudos. Unidades públicas también (`Meters`, `Seconds`, `MetersPerSecond`,
`Radians`). `Vec2`/`Vec3` pueden usarse internamente si la API deja clara
coordenada y unidad.

## Representación

`VisualOf(Entity)` enlaza una representación a la entidad autoritativa.
Primitiva, low-poly, replay y visual remoto son consumidores equivalentes.
Ningún perfil o regla contiene rutas de assets.

