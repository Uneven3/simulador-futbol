# Ahora

Qué se está haciendo, no qué se hizo (ley 15): la historia está en `git log` y
las cifras en `measurements/`. **Techo: 40 líneas hasta la deuda declarada.**

## Objetivo activo

**MVP 2 — Partido reglamentariamente completo.** Sustituciones, faltas,
ventaja, disciplina, tiros libres, penales, dropped ball y tanda (`NORTE.md`).

## Empezar aquí

**Cuerpos antes que reglas.** Lo que bajó las faltas de 168 a 42 no fue una
regla del árbitro sino un dato del cuerpo, y lo que queda de exceso —42 contra
22— tampoco se arregla arbitrando: un jugador pasa de parado a siete metros por
segundo en un tick, no frena, no se compromete a nada y no protege el balón. El
reglamento describe cuerpos, así que hasta que los haya cada regla nueva se
juzga sobre piezas.

1. **Girarse y salir**: es lo que le falta a proteger el balón. La regla
   —al balón no se llega a través de un cuerpo— ya está; ponerse en medio sin
   salida deja al portador clavado y el partido sin un solo tiro.
2. **El defensor está delante y no hace nada más que estar.** Ahora bloquea el
   tiro si se cruza, pero no se cruza a propósito: no sale a achicar el ángulo
   ni mete la pierna. Sigue habiendo cuatro veces los goles reales.
3. **Nadie se recupera nunca**: la fatiga solo baja dentro del partido, y el
   descanso entre partes no repone nada.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Cuatro de cada diez tiros son gol** (real, uno de cada diez), y de ahí sale
  un ritmo de gol que sigue triplicando el real: nada de esto puede presentarse
  como predicción. No es que el atacante llegue solo —el rival más cercano está
  a 2,2 m y eso no ha cambiado nunca—, es lo que ese rival hace: ver el punto 2.
- **16 faltas por 90 minutos contra las ~22 reales.** El criterio pasó de
  sobrar a faltar cuando los cuerpos tuvieron inercia, y no se toca hasta que
  haya protección del balón: el número que hoy cuadre volvería a descuadrar.
- **La simetría solo se afirma sobre la posesión.** Goles y tiros de doce
  partidos no dan muestra para más que una banda de tres sigma, que atrapa un
  sesgo grosero y nada fino. Medirla de verdad pide un barrido de cien partidos.
- **El fuera de juego no se pita nunca**: un anotado no puede disputar el balón
  (`select_ball_challenger` lo salta), así que la regla es un campo de fuerza y
  el silbato de `referee_offside_system` no suena.
- `Ball.predictions` y `PlayerReading` son omniscientes: hoy todos leen el
  futuro real, y en MVP 4 deja de ser válido.
- `SimulationSet` refleja el orden del original; el pipeline semántico de
  `ARCHITECTURE.md` sigue pendiente.
- `PLAYER_HEIGHT` es una constante que se copia a `Attributes`, no un dato por
  jugador.
- Faltan los overlays sin dato: campo visual, observaciones y memoria (MVP 4), y
  responsabilidades tácticas más allá de la designación (MVP 5).
- Los canales `Formation` y `Performance` existen en el hub y casi no tienen
  productores: `Formation` sólo emite carreras de ataque, `Performance` nada.
- **Allocations por tick** (ley 12): 18 `collect()`/`vec!` entre
  `player_decisions`, `team_tactics` y `player_movement`. Importa para MVP 6.
- **Lo visual funciona pero es pobre**: la cámara ve una fracción del campo, el
  HUD no tiene fondo, no hay meshes de portería.

## Reparto previsto de atributos de jugador

**Un atributo entra cuando tiene mecanismo que lo lee, unidad real y referencia
que lo calibra.** MVP 3, los motores (velocidad, aceleración, frenado, giro,
fatiga, alcance); MVP 4, los perceptivos (campo visual, latencia, atención,
memoria); MVP 5, los tácticos (familiaridad, disciplina, riesgo); MVP 6 los
vuelve editables para comparar variantes.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
- No añadir un atributo de jugador sin mecanismo, unidad y referencia.
- No añadir un canal de diagnóstico encendido por defecto.
