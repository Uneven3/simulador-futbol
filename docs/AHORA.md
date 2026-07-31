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

1. **Aceleración, frenado y giro** (MVP 3): lo que hoy es instantáneo, con
   unidad real y referencia que lo calibre.
2. **Compromiso motor** (`ActionCommitment`): una acción dura, tiene fases y no
   se cancela gratis.
3. **Disciplina**: amonestación y expulsión sobre el hecho que ya se publica.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **42 faltas por 90 minutos contra las ~22 reales**, y el criterio no puede
  bajar de ahí solo: sin inercia ni protección del balón, la mitad de lo que
  parece una entrada es geometría.
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
- **El ritmo de gol es irreal**, en un orden de magnitud: nada de aquí puede
  presentarse como predicción hasta calibrar, que es un hito tras MVP 4.
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
