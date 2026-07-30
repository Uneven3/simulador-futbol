# Referencia histórica — simulación durante el port

> “HOY” se refiere a la sesión fechada dentro del documento. El trabajo activo
> vive en `../../AHORA.md`.

# Cómo funciona la IA de simulación HOY (guía de lectura)

Escrito 2026-07-11 para retomar el trabajo. Complementa
`ai-arquitectura-original-vs-port.md` (original vs port, conflictos C1-C11) y
`eliza-controller-port.md` (tabla de qué se portó de dónde).

## Observación del usuario pendiente de resolver

> "Los enemigos se están dando pases" — los pases de un equipo terminan en los
> pies del rival con tanta frecuencia que parece deliberado.

Es la manifestación visual del problema de intercepciones (~13 pases cortados
por minuto, medido). La cadena causal, ya diagnosticada con el forense de
`long_match_stats`:
1. El pasador elige el pase con un modelo de odds que asume que los rivales
   siguen su inercia (proyección 0,2-0,3 s) — pero
2. el rival ve la trayectoria REAL de la pelota en el mismo tick del pase
   (sin retardo de reacción, conflicto **C4**), y
3. corre a la intercepción sin costo de arranque ni giro (cinemática
   instantánea + modelo de tiempo optimista, conflicto **C3**), y
4. el pase no se re-apunta al momento del toque (conflicto **C5**).
El resultado: pases que el modelo consideraba seguros son cazados por un
interceptor sobrehumano. Arreglo planeado: C3+C4 (aceleración finita +
MentalImage con retardo), no más parches de potencia/odds — ya se probó que
no mueven la aguja.

## El pipeline de un tick (100 Hz, FixedUpdate)

Orden en `simulation/mod.rs` (espejo de `Match::Process()`):

```
SimulationSet::Players
  1. update_possession_designation   (player_movement.rs)
  2. team_ai_update                  (team_ai.rs)
  3. eliza_movement_system           (eliza.rs)
  4. apply_player_velocity           (player_movement.rs)
  5. resolve_player_overlap          (player_movement.rs)
SimulationSet::Kicks
  6. player_kick_system              (player_movement.rs)
SimulationSet::BallCollisions
  7. ball_body_collisions            (ball_collisions.rs)
SimulationSet::BallPhysics
  8. ball_process                    (ball_physics.rs)  ← integrador real
SimulationSet::Referee
  9. referee_system / offside / set_piece (referee.rs)
```

### 1. Designación de posesión
Por equipo, el jugador que llega antes a la trayectoria predicha de la pelota
(`find_interception` sobre `ball.predictions`, 3 s a pasos de 10 ms). Solo ÉL
va a la pelota. El poseedor actual siempre es el designado de su equipo. El
receptor de un pase en vuelo (`MatchState.pass_target`) tiene prioridad
mientras la pelota viaje a >0,3 m/s y él llegue en <3,5 s. Jugadores en
registro de offside quedan excluidos.

### 2. IA de equipo (`TeamAis`, port de TeamAIController)
Cada tick: possession amounts (con fading a 0,5%/tick), línea de offside trap
(deepestDanger: pelota, predicción a 700 ms, rival designado, compañero
rezagado), lista de rivales peligrosos, man marking (3 más peligrosos →
mejor marcador por `marking_quality`), corridas de ataque (cada 500 ms),
forward support player (cada 1500 ms), offensiveness bias (marcador + tiempo).

### 3. Movimiento por jugador (`eliza_movement_system`)
Árbol de decisión por jugador:
- **Set piece activo** → congelado (los saques ejecutados por jugadores son
  pendiente de Fase 4).
- **Arquero** → `goalie_movement`: biseca el ángulo pelota-palos sobre la
  línea de cobertura; sale a achicar en 1v1 (con evaluación de peligro
  secundario); si `calculate_ball_bound_for_goal` → cubre el punto de cruce;
  recoge pelotas lentas en su área a <8 m. **No tiene atajadas (C1).**
- **Poseedor** → `carry_movement`: persigue la pelota si está a >0,5 m; si la
  tiene al pie, corre en la dirección del force field de gambeta (repelido por
  los 5 rivales más cercanos y las líneas, atraído al arco). Camina (5 m/s) en
  tráfico, esprinta (7,6) libre.
- **Designado con pelota ganable** (`ball_winnable`: possessionAmount>0.99, o
  pelota suelta con >0.5) → `to_ball_movement` al punto de intercepción.
- **Todos los demás** (incluido el designado SIN pelota ganable — clave, ver
  §6b del doc de arquitectura):
  1. Si el rival portador está cerca (umbral por mindset: 10-20 m × factores)
     y soy uno de los 2 más cercanos → hunt: jockey en `get_defend_position`
     (bisectriz perpendicular sobre la línea rival→mi arco), solo si
     `need_defending_movement`.
  2. Si no → estrategia por línea: posición de formación adaptada
     (`get_adapted_formation_position`: bloque dinámico con focos micro/
     midfield/side según posesión) mezclada con la posición de soporte
     (force field: base + repulsión de rivales/compañeros/pelota + atracción
     al poseedor + corridas), más componente defensivo (marca personal via
     `add_defensive_component`) y offside trap. Velocidad final pasada por
     `get_lazy_velocity` (pereza por rol, distancia a la acción y aliento).

### 6. Acciones con la pelota (`player_kick_system`)
Mecánica de toques discretos (la pelota NUNCA se pega al jugador):
- **Pickup/robo**: contacto a <0,65 m (receptor de pase: 1,1 m de alcance con
  ventaja de 0,45). Robo al poseedor solo si: designado + <0,5 m + más cerca
  de la pelota que el poseedor (×0,8) + pelota "robable" (>1 m del pie o
  retenida >2 s) + cooldowns (500/1000 ms). Touch bias: el último tocador
  (≤1,5 s, <1 m) gana los empates salvo margen de 0,25 m.
- **Decisión** (a los 150 ms de recibir; gambeta a los 350 ms): port de
  `GetOnTheBallCommands` con prioridad **pánico → pase → tiro → gambeta**:
  - Pase: rating táctico del compañero (espacio adelante/espacio/cercanía al
    arco con pesos por mindset) debe superar el mío + umbral; odds del pase
    por línea (port de `_GetPassingOdds` de Eliza); tipo short/long/high por
    mejores odds; `pass_minimum` +0,15 sobre el original (compensación).
  - Tiro: factor de posición ideal (a ~7 m del área), odds a 3 puntos del
    arco, `odds^0.5 + random(0,0.5) > 0.5`.
  - Pánico: defensores cerca del propio arco sin salida → reventarla.
  - Acorralado (2+ rivales a <3 m): mejor pase de escape con odds >0,2 o
    pánico.
- **Ejecución**: pases con `solve_pass_momentum` (bisección de la velocidad
  inicial contra el integrador real para que llegue al receptor con ritmo);
  tiros con la receta tuneada (línea de gol ±55, curl al segundo palo,
  topspin 12); gambeta = knock en dirección del force field a ritmo del
  portador (gambeta corta en tráfico).

### 8-9. Física y árbitro
La pelota se integra con el port fiel de `Ball::CalculatePrediction` (la
predicción ES la realidad, invariante del original). El árbitro: gol barrido,
out con pelota completa, offside (registros al toque), reanudaciones con
timer (pelota colocada al pitar). **Los saques teletransportan a todos a la
formación base (C2)** y no hay faltas ni reloj (C10).

## Estado de métricas (fin de sesión 2026-07-11)
41,6 → 19,6 robos/min a lo largo de la sesión; 0 forcejeos cuerpo a cuerpo;
rachas de posesión hasta 23 s; PERO ~13 pases/min terminan en el rival (la
observación de arriba) y salen demasiados goles (arquero sin atajadas, C1).

## Plan acordado para mañana
1. **C1**: atajadas del arquero (deflect por reglas con las predicciones).
2. **C2**: saques contextuales (port de `PrepareSetPiece`; la función
   `adapted_formation_position` ya acepta todos sus parámetros de foco).
3. **C3+C4**: aceleración finita de jugadores + retardo de reacción
   (MentalImage) → esto es lo que arregla "los enemigos se dan pases".
Herramientas: `cargo test long_match_stats -- --ignored --nocapture` (métricas,
forense de intercepciones, snapshots ASCII de la cancha cada 30 s); la
verificación visual siempre la hace el usuario con `cargo run`.
