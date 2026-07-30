# Referencia histórica — IA original vs port

> Fuente de algoritmos y fallos conocidos; no define la arquitectura actual.
> Ver `../../NORTE.md`.

# Cómo funciona la IA: el original (C++) vs nuestro port — y por qué dos jugadores pelean la pelota eternamente

Escrito 2026-07-11 tras el diagnóstico visual del usuario: "básicamente 2
jugadores pelean la pelota constantemente, y siempre ha sido el mismo problema
desde el principio del port". Este documento explica la arquitectura de ambos
lados y señala la causa estructural. **Es el documento de arranque para quien
retome el problema.**

---

## 1. La IA del original tiene CUATRO capas, y nosotros portamos solo dos

```
┌────────────────────────────────────────────────────────────┐
│ 1. TeamAIController (por equipo, táctica)                  │  ✅ portada (team_ai.rs)
│    posesión, línea de offside, formación adaptada,         │
│    marcas, corridas de ataque                              │
├────────────────────────────────────────────────────────────┤
│ 2. ElizaController (por jugador, decisión)                 │  ✅ portada (eliza.rs)
│    a dónde moverme, a quién pasar, cuándo tirar            │
│    → emite PlayerCommands (deseos, no acciones)            │
├────────────────────────────────────────────────────────────┤
│ 3. Humanoid / animaciones (por jugador, EJECUCIÓN)         │  ❌ NO portada
│    elige una animación de cuerpo completo (~1000 anims     │
│    de mocap) que satisfaga el comando; la animación        │
│    mueve al jugador Y toca la pelota                       │
├────────────────────────────────────────────────────────────┤
│ 4. Física de cuerpos (colisión jugador-jugador real)       │  ❌ NO portada
│    los cuerpos no se atraviesan; el que lleva la pelota    │  (solo separación
│    la tapa con el cuerpo                                   │   posicional suave)
└────────────────────────────────────────────────────────────┘
```

Las capas 1-2 (lo que portamos) son el **cerebro**. Las capas 3-4 (lo que no
portamos) son el **cuerpo**. El duelo por la pelota se resuelve en el CUERPO,
no en el cerebro. Por eso todos los parches a nivel de decisión (cooldowns,
shielding por reglas, touch bias, pases de escape) atenúan síntomas pero no
eliminan el problema.

## 2. Cómo resuelve el original un duelo por la pelota

En el original (`humanoid.cpp`, `humanoid_utils.cpp`, comandos de
`playercontroller.cpp`):

1. **Compromiso temporal**: cada acción es una animación con duración
   (200-800 ms). Un jugador que inicia un trap, un quite o un giro queda
   COMPROMETIDO hasta que la animación termina. No puede re-decidir cada 10 ms.
2. **El toque es un evento ganado por una animación**: para tocar la pelota, el
   sistema busca una animación cuyo pie pase por donde va a estar la pelota
   (`GetBestCheatableAnim`). Si dos jugadores disputan, UNO llega con animación
   válida y toca; el otro **falla y queda en recovery** — ese whiff de ~500 ms
   es lo que resuelve el duelo. El ganador se va con la pelota mientras el
   perdedor se recupera.
3. **El cuerpo tapa**: colisión física real. El portador se interpone; el
   defensor no puede atravesarlo para llegar a la pelota. El shielding es
   GRATIS, emergente de la física.
4. **El defensor designado NO persigue la pelota: contiene.** Esto es clave y
   está en `_MovementCommand` (playercontroller.cpp, rama "OTHER TEAM IS IN
   BALL CONTROL"): cuando el rival tiene el control, el designado defensivo
   corre a un punto EN LA LÍNEA rival→arco (`GetDefendPosition`, jockeying),
   imita el movimiento del rival (man-marking mimicry), y solo intenta ganar la
   pelota en eventos discretos y condicionados:
   - `_InterfereCommand`: solo si `CouldWinABallDuelLikeliness() >= 0.2`
     (ángulo favorable entre él, la pelota y el rival);
   - `_SlidingCommand`: solo si likeliness >= 0.7, la pelota a 0.7-1.6 m del
     rival y el rival tarda >260 ms en llegar a ella;
   - `_TrapCommand`: solo si su tiempo a la pelota < 1000 ms Y el del rival
     > 400 ms Y el rival NO tiene posesión.
   Es decir: el original espera el ERROR (toque largo, pase desviado) y recién
   ahí ataca la pelota. Mientras tanto, acompaña.
5. **La posesión es emergente, no un lock**: `HasPossession()` se deriva cada
   tick de la distancia y compatibilidad de movimiento con la pelota. No hay
   "dueño" explícito ni cooldowns de robo: hay pies que llegan o no llegan.

## 3. Cómo funciona nuestro port hoy

- Sistemas Bevy a 100 Hz en `FixedUpdate`, orden: Players → Kicks →
  BallCollisions → BallPhysics → Referee (`simulation/mod.rs`).
- **Cinemática pura**: cada jugador es un punto con `Velocity` que puede
  cambiar de dirección y módulo COMPLETO cada 10 ms. Sin inercia, sin
  compromiso, sin animaciones, sin recovery. Reacción perfecta e instantánea.
- **Posesión como lock explícito** (`MatchState.possession_player`) con reglas
  inventadas para compensar la falta de cuerpo: radio de contacto 0.65 m,
  cooldowns de robo (500/1000 ms), shielding por regla (rival debe estar más
  cerca de la pelota que el poseedor × 0.8 y pelota "robable"), touch bias
  (último tocador gana empates ≤1.5 s / 1 m / 0.25 m de margen).
- **Toques discretos** vía `touch_ball` (nunca teleport) — esto SÍ es fiel al
  original.
- Capa de decisión (eliza.rs + team_ai.rs): port fiel de formaciones
  adaptadas, force field de soporte, lazy velocity, marcas, arquero,
  panic/pase/tiro/gambeta. Ver `eliza-controller-port.md`.
- **PERO nuestro `to_ball_movement` hace que el designado defensivo corra al
  punto de intercepción de la pelota SIEMPRE, a sprint, incluso cuando el
  rival la tiene controlada.** (eliza.rs: `is_designated` → `to_ball_movement`
  sin condición de contención). El hunting (`get_defend_position`) existe pero
  solo aplica a los NO designados.

## 4. Por qué emergen "2 jugadores peleando la pelota constantemente"

Combinación letal de tres ausencias:

1. **Sin compromiso temporal**: ambos contendientes ajustan su vector
   perfectamente cada 10 ms → ninguno comete el error que en el original
   resuelve el duelo. Es un empate perpetuo de dos controladores óptimos.
2. **Sin contención defensiva**: nuestro designado defensivo ataca la PELOTA
   (no al rival) sin parar. En el original estaría a 1-2 m, goal-side,
   esperando el toque largo. En el nuestro está encima del portador siempre,
   y con la separación de cuerpos simétrica ambos se empujan (la deriva lenta
   hacia afuera que se ve en pantalla).
3. **Sin cuerpo**: no hay tapada física; nuestras reglas de shielding evitan
   el robo instantáneo pero no evitan que el defensor esté SIEMPRE a distancia
   de duelo, pegado, generando el forcejeo visual.

Los números lo confirman: tras los parches de hoy los robos bajaron de 41.6 a
16.2 por minuto y los quites de 190 a 6 por partido de 10 min — pero el
usuario sigue viendo el forcejeo porque **el defensor sigue pegado al portador
todo el tiempo aunque ya casi nunca le robe**. El problema visual es la
DISTANCIA y la persistencia del contacto, no la frecuencia de robo.

## 5. Opciones para arreglarlo de verdad (a decidir, sin código aún)

Ordenadas por relación impacto/esfuerzo:

1. **Contención defensiva (port fiel de la rama defensiva de
   `_MovementCommand`)** — el designado defensivo NO va a la pelota cuando el
   rival la controla: va al punto de jockey (`GetDefendPosition`, ya portado
   pero solo usado por cazadores) y solo ataca la pelota cuando se cumplen las
   condiciones del original (toque largo del rival: pelota a >1 m de sus pies;
   ángulo favorable `CouldWinABallDuelLikeliness`; ventanas de tiempo
   `TimeNeededToGetToBall` propio vs rival). Esto reproduce la conducta visible
   "defensor acompaña a 1-2 m" y elimina el forcejeo permanente. **Es la
   opción recomendada y la más fiel al original.**
2. **Compromiso temporal mínimo (sub-port de la capa humanoid)** — sin
   animaciones: cada intento de jugar la pelota (quite, trap, toque) dura
   ~300-500 ms durante los cuales el jugador no re-decide, y si falla queda
   200-400 ms en recovery a velocidad reducida. Introduce el "whiff" que
   resuelve duelos. Complementa (1).
3. **Cuerpos con colisión real (Avian)** — ya estaba planeado para Fase 3-4
   (la decisión híbrida: pelota analítica + cuerpos físicos). El shielding
   pasa a ser emergente y las reglas artificiales de robo se pueden borrar.
   Más esfuerzo; es la solución definitiva junto con (2).
4. Micro-ajustes de las reglas actuales (cooldowns, radios) — **agotado**: ya
   se demostró que mueve los números pero no cambia lo que se ve.

## 6. Actualización: opción 1 IMPLEMENTADA (misma sesión, 2026-07-11)

Se implementó la **contención defensiva** (`eliza.rs::designated_movement` +
`contain_movement`): el designado va a la pelota solo si
`possessionAmount > 0.99` o pelota suelta con `> 0.5` (condiciones del magnet
original); si el rival controla, hace jockey con el blend
manMarkingBias/huntTarget del original. Además: el árbitro ahora coloca la
pelota en el punto de reanudación al pitar (antes seguía rodando fuera de la
cancha durante el timer del saque), y el toque de gambeta en tráfico baja a
ritmo de gambeta (3.5 m/s).

Resultado en 2 min de juego real: 60 posesiones, 47 pases (24 S/12 L/11 H),
10 panic clears, **0 quites/forcejeos**. Métricas 10 min: 26.3 flips/min todos
"loose" — el problema restante ya NO es el forcejeo sino la **tasa de
intercepción de pases** (nuestra ejecución de pase con potencia heurística +
receptores que no atacan el pase hacen que ~1 de cada 2 pases cambie la
posesión; el original resuelve dirección/potencia con AI_GetPass y el receptor
corre al pase). Ese es el siguiente candidato si el usuario aún ve poco juego:
portar AI_GetPass + prioridad del receptor en la designación.

## 6b. Segunda ronda (misma sesión): pases y presión

Con la contención, el problema pasó a los pases: forense midió **251/318
pérdidas tras pase**, 90% "en route". Cadena de causas encontrada con el log
de intercepciones (u a lo largo de la línea + rol del interceptor):

1. Los interceptores eran los CF/CB que PRESIONABAN al pasador — mi
   `contain_movement` pegaba al designado defensivo 1-2 m del portador
   SIEMPRE, acorralando a los centrales, forzando pases de escape y
   cortándolos ellos mismos. **El original no contiene así**: el designado
   defensivo sin urgencia tiene autoBias ≈ 0 y juega su ESTRATEGIA (formación
   + hunt condicional por mindset/NeedDefendingMovement). Arreglo: designado
   sin pelota ganable = jugador off-the-ball común (`ball_winnable` +
   fallthrough). Pérdidas por pase 240→159, flips 25.5→19.6/min.
2. `solve_pass_momentum` (ball_physics.rs): potencia del pase resuelta por
   bisección contra el integrador real (antes heurísticas que morían cortas).
3. Receptor: prioridad de designación (`MatchState.pass_target`, expira con
   pelota casi parada <0.3 m/s — a 1.0 congelaba equipos) y alcance de trap
   extendido 1.1 m (las anims del original estiran la pierna).

**Instrumentación permanente en long_match_stats**: turnovers por tipo de
salida [none, pass, knock, clear/shot], pases perdidos en recepción vs en
ruta, log de 20 intercepciones (u, lane_dist, rol), snapshots ASCII.

Pendientes conocidos tras esta ronda: (a) todavía ~13 pérdidas de pase/min
(en el original la ejecución la refina AI_GetPass al momento del toque);
(b) salen demasiados goles (5-2, 7 kickoffs en 10 min) — la defensa quedó más
pasiva; revisar arquero/hunt cerca del área; (c) compromiso temporal
(whiff/recovery) y cuerpos Avian siguen siendo el arreglo de fondo.

## 8. Auditoría de puntos de conflicto restantes (2026-07-11, código nuestro vs original)

Comparación sistemática módulo a módulo. Ordenados por impacto estimado en el
gameplay. Los ítems marcados 🎯 son los candidatos inmediatos.

### 🎯 C1. El arquero no tiene mecanismo de atajada
Original: `_KeeperDeflectCommand` (playercontroller.cpp) encola anims de
`e_FunctionType_Deflect` — estirarse, palmear, embolsar — con reglas (no usar
manos tras pase intencional de compañero, solo dentro del área, ventanas de
posesión). **Es EL mecanismo de atajada del juego.** Nuestro arquero solo se
posiciona (goalie_movement) y recoge pelotas lentas a <8 m; un tiro al arco
solo se "ataja" si choca su cápsula de colisión (0.35 m de radio) por
casualidad. Consecuencia directa: los 7-9 goles por 10 min que estamos viendo.
Arreglo propuesto: sistema de deflect por reglas — si la pelota cruza cerca del
arquero (predicciones) dentro de su alcance (~1.5-2 m, más con estirada),
tocarla hacia afuera (touch_ball con dirección de despeje) con probabilidad por
dificultad/stats.

### 🎯 C2. Los saques teletransportan a TODOS a la formación base
`referee_set_piece_system` (referee.rs:263) resetea a los 22 jugadores a su
posición base en CADA reanudación. El original (`PrepareSetPiece`,
teamAIcontroller.cpp:653-905) posiciona por contexto: córner = poblar el área
(xFocus en el área, midfieldFocus alto), lateral = posiciones locales alrededor
del punto, tiro libre = barrera de 3 hombres a 9.15 m, penal = fuera del área.
Consecuencia: cada lateral "resetea el partido" visualmente y ningún córner
genera peligro. Port directo posible: `adapted_formation_position` ya está
portada con todos los parámetros de foco que usa PrepareSetPiece.

### 🎯 C3. Modelo de tiempo-a-la-pelota demasiado optimista y sin física
Original `AI_GetTimeNeededForDistance_ms`: para distancias grandes usa
`dist / (maxVelocity * 0.75)` (25% de pesimismo) y para cercanas SIMULA
aceleración, frenado y giro paso a paso (incluye offset del pie). Nuestro
`find_interception`: `dist / speed + 0.05` a velocidad tope — todos llegan
antes de lo que llegarían, sin costo de giro ni arranque. Afecta: designación,
possessionAmount, ball_winnable, carreras de intercepción (contribuye a los
~13 pases cortados/min: el interceptor "sabe" que llega y efectivamente llega
porque su cinemática tampoco tiene aceleración). Arreglo coherente: usar
maxVelocity*0.75 para >16 m y añadir un costo de arranque/giro simple, Y
al mismo tiempo darle a los jugadores aceleración finita en
apply_player_velocity (hoy la velocidad cambia instantáneamente).

### 🎯 C4. Sin MentalImage (retardo de reacción)
Original: cada controller ve el estado con `GetReactionTime_ms()` de retardo
(~50-200 ms + hasta 100 ms por dificultad; el toque propio se ve al instante).
Nosotros: información perfecta e instantánea para todos. Los interceptores
reaccionan al pase EN EL TICK en que sale. Es una de las dos razones de fondo
de las intercepciones (con C3). Port razonable: buffer circular de snapshots
de 300 ms en un Resource; cada controller lee el snapshot de su retardo.

### C5. Ejecución de pase sin refinamiento en el momento del toque
Original: `AI_GetPass` se llama al DECIDIR y se RE-llama al momento físico del
toque ("getpass is recalled at moment of passing, for refinement") con la
posición actualizada del receptor. Nuestro pase fija el aim en la decisión
(hasta 350 ms antes del contacto). Además `AI_GetShotDirection` (elección
continua del punto del arco según ángulo) está aproximada con 3 puntos fijos.

### C6. Condiciones de duelo reemplazadas por reglas de lock
Original: trap/interfere/sliding tienen gates finos — `CouldWinABallDuelLikeliness`
(ángulo rival-pelota-yo), `oppTimeNeededToGetToBall > 400 ms`, ventanas de
`AllowLastDitch`. Nuestro sistema de posesión: lock explícito + cooldowns +
shielding por regla + touch bias. Funciona, pero es mecánica inventada;
cuando lleguen los cuerpos (Avian) hay que revisitarla entera.

### C7. Sin roles dinámicos (Hungarian)
`CalculateDynamicRoles` reasigna las posiciones de formación al jugador más
cercano (algoritmo húngaro, cada pocos ticks). Sin esto, un LB que quedó en
punta tras un córner cruza TODA la cancha de vuelta a su posición mientras el
RM cubre su hueco solo por accidente. Visible como jugadores haciendo
maratones diagonales.

### C8. Sin aceleración ni orientación corporal en los jugadores
Original: humanoid con momentum físico — cambiar de dirección cuesta tiempo
(anims de giro), correr hacia atrás es lento, el cuerpo tiene facing.
Nosotros: `Velocity` teleporta de dirección en 10 ms. Las cápsulas no muestran
facing así que visualmente aún no se nota, pero afecta todos los duelos y
carreras (ver C3). Cuando haya cuerpos/meshes esto será lo más visible.

### C9. Stats y fatiga congelados
`fatigueFactorInv` = 1 siempre (GetLazyVelocity, hunt threshold), `technical_*`
= 0.5, `GetMatchDifficulty` = 1.0 implícito (más presión que el default del
original). El original degrada velocidad/decisiones con el cansancio.

### C10. Árbitro parcial
Faltan faltas, tarjetas, penales y ventaja (referee.cpp tiene
`CheckFoul`/advantage); no hay reloj de partido ni mitades (MatchPhase existe
pero nadie lo avanza). El offside no distingue "interfiere con el juego" (el
original tampoco mucho).

### C11. Detalles menores
- `GetSupportPosition` (la versión candidatos+ratings de Eliza) no portada —
  solo usamos la versión force field (el original usa ambas según contexto...
  en realidad solo llama a la force field desde las estrategias; OK).
- Celebraciones, mirar al árbitro, throw-in con las manos: cosmético, falta.
- `possessionAmount` por jugador usa tiempos de EQUIPO (aprox.).
- `ApplyKeeperRush` (salida del arquero comandada) sin trigger — igual que el
  team pressure del original (también sin trigger allí).
- Micro: `oneTouchIsHard` usa technical_shortpass fijo 0.5; kickoff del
  original coloca 2 apoyos en el círculo (nosotros: taker solo).

## 7. Estado de métricas antes de la contención (referencia histórica)

`cargo test long_match_stats -- --ignored --nocapture` (10 min simulados,
imprime snapshots ASCII de la cancha cada 30 s + forense de robos):
score 3-2, 16.2 robos/min (real: ~5-8), 6 quites directos, viaje medio de la
pelota entre robos 10.6 m, racha máxima de posesión 19.8 s, laterales 5,
corner 1. Formaciones 4-4-2 visibles y sanas en los snapshots; el problema
restante es el comportamiento de duelo descrito arriba.
