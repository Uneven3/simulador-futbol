# Contrato de las reglas del fútbol

## Autoridad

- The International Football Association Board.
- Edición base: **Laws of the Game 2026/27**.
- Inglés autoritativo ante divergencias.
- `CompetitionRules` declara modificaciones y protocolos.

Fuentes:

- <https://www.theifab.com/laws/latest/>
- <https://www.theifab.com/law-changes/latest/>
- <https://www.theifab.com/laws-of-the-game-documents/>

Este inventario no reemplaza el texto IFAB.

## Capas

```text
LawDefinition → PhysicalFact → PotentialIncident
              → RefereeObservation → RefereeDecision → MatchTransition
```

Al inicio puede usarse `PerfectOfficiating`; la separación permanece.

## Cobertura

`Catalogued`, `ScenarioDefined`, `Implemented`, `Validated`, `Configured`,
`Subjective`, `Deferred`, `PresentationOnly`.

Cada requisito implementable tendrá ID (`LAW09.OUT.001`), referencia, edición,
precondiciones, hechos, decisión/transición, restart/sanción, parámetros,
escenarios de frontera y cobertura. Los escenarios corren headless y con
primitivas.

## Laws 1–17

| Ley | Requisitos del simulador | Estado aproximado | MVP |
|---|---|---|---|
| 1 Field | Superficie, marcas, dimensiones, áreas, arcos, postes, banderines, área técnica | Geometría parcial | 1 |
| 2 Ball | Dimensiones, masa/presión, defecto, reemplazo y balones extra | Radio/dinámica; sin contrato completo | 1–2 |
| 3 Players | 11, goalkeeper, mínimo 7, sustituciones, expulsados y personas extra | Sustituciones en detención, máximo configurable y corte bajo mínimo; sin personas extra | 2 |
| 4 Equipment | Obligatorio/prohibido, seguridad, colores, infracciones y retorno | Ausente | 2/config |
| 5 Referee | Autoridad, ventaja, disciplina, lesiones, interferencia, reloj y correcciones | Reloj, ventaja y disciplina acumulativa; faltan severidad, lesiones e interferencias | 2 |
| 6 Officials | Asistentes, cuarto oficial, adicionales, VAR/AVAR | Ausente | diferido |
| 7 Duration | Mitades, descanso, añadido, recuperación, penal extendido, abandono | Mitades, descanso, añadido y tiempo suplementario como dato de competición; sin abandono | 1–2 |
| 8 Start/restart | Sorteo, kick-off, posiciones, balón en juego, dropped ball | Cambio de mitades y kick-off del otro equipo, con escenario; alguien ejecuta cada reanudación y los rivales guardan distancia; dropped ball implementado | 1–2 |
| 9 In/out | Cruce completo, detención arbitral, contacto con oficial y excepciones | Parcial | 1 |
| 10 Outcome | Gol, ganador, empate, extra time y tanda | Marcador, tiempo suplementario y tanda determinista/configurable; falta contrato de ganador de competición | 1–2 |
| 11 Offside | Posición, toque, interferencia, advantage, deliberate play/save, excepciones | Simplificado | 2 |
| 12 Fouls | Direct/indirect, handball, goalkeeper, severidad, DOGSO y disciplina | Contacto, ventaja, sanción y tarjetas acumulativas; sin mano, severidad ni DOGSO | 2 |
| 13 Free kicks | Tipo, señal, posición, distancias, muro, quick kick, doble toque | Concesión construida y apagada; distancia de rivales sí, sin muro ni tipos | 2 |
| 14 Penalty | Posiciones, procedimiento, feint, goalkeeper e infracciones combinadas | Enum sin ejecución | 2 |
| 15 Throw-in | Concesión, cuerpo/manos, lugar, distancia, doble toque e infracciones | Simplificado | 1–2 |
| 16 Goal kick | Concesión, posición, balón en juego, oponentes y doble toque | Simplificado | 1–2 |
| 17 Corner | Concesión, área, distancia, balón en juego y doble toque | Simplificado | 1–2 |

## Protocolos/configuración

- sustitutos, oportunidades y concussion substitutions;
- return substitutes y temporary dismissals;
- variantes youth/veterans/disability/grassroots;
- extra time y kicks from the penalty mark;
- time-limited substitution;
- off-field treatment and assessment;
- throw-in/goal-kick countdown;
- “only the captain”;
- VAR y ensayos IFAB explícitos.

Pertenecen a `CompetitionRules`, no a “fútbol estándar” implícito.

## Objetivo frente a subjetivo

Objetivo: cruce completo, gol geométrico, posiciones, elegibilidad, tiempo y
distancias.

Subjetivo: careless/reckless/excessive force, interferencia, deliberate play,
advantage, handball, conducta y DOGSO. Requieren `RefereePolicy`, no umbrales
anónimos.

## Escenarios iniciales

MVP 1 — estado (catálogo en `src/scenarios.rs`, suite en `tests/scenarios.rs`):

| Escenario | Cobertura |
|---|---|
| gol/no-gol por postes y travesaño | `shot_off_the_post`, `shot_off_the_crossbar` |
| cruce incompleto | `ball_stopping_on_the_goal_line` |
| side netting | frontera unitaria (`test_no_goal_through_side_netting`), sin escenario |
| touchline/goal line | `ball_over_the_touchline`, `ball_over_the_opponents_goal_line`, `ball_over_own_goal_line` |
| a alta velocidad | `goal_at_high_speed` (0,4 m por tick, sin tunneling) |
| throw-in vs goal kick/corner | los tres escenarios anteriores |
| kick-off legal/ilegal | **ausente**: no hay reglas de posición ni de balón en juego para el saque (MVP 2) |
| reloj, descanso/final | `short_match` (fases PreMatch→FirstHalf→HalfTime→SecondHalf→FullTime) |
| misma escena headless y con primitivas | `tests/layer_boundaries.rs` |

MVP 2:

- matriz de offside y excepciones;
- direct/indirect, advantage y disciplina;
- handball/restricciones del goalkeeper;
- penal e infracciones combinadas;
- sustitución, expulsión y mínimo de siete;
- extra time y tanda.

## Definición de completo

1. Cada cláusula relevante 2026/27 tiene ID/cobertura.
2. Transiciones deterministas tienen fronteras.
3. Juicios subjetivos declaran inputs/política.
4. Variantes son datos.
5. No hay reglas ocultas en IA, presentación o animación.
6. Cada escenario genera trace explicable.
