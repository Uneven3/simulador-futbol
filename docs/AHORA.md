# Ahora

Qué se está haciendo, no qué se hizo (ley 15): la historia está en `git log` y
las cifras en `measurements/`. **Techo: 40 líneas hasta la deuda declarada.**

## Objetivo activo

**MVP 2 — Partido reglamentariamente completo.** Sustituciones, faltas,
ventaja, disciplina, tiros libres, penales, dropped ball y tanda (`NORTE.md`).

## Empezar aquí

1. **La reanudación se ejecuta.** `referee_set_piece_system` recoloca a los dos
   equipos en formación base —un espejo exacto— y suelta el balón sin dárselo a
   nadie. El empate lo rompen tres desempates que van todos para el local (los
   `<=` de `designated_player_overall` y `team_tactics`, y el `<` estricto de
   `select_ball_challenger`). Por eso `two_identical_teams_finish_level` está
   roja: 7-0 sobre seis partidos. Se arregla dando el balón a quien saca:
   portero en saque de puerta, jugador de campo más cercano en el resto; a 0,4 m
   del balón (`ball_at_feet_distance` es 0,7); rival apartado 9,15 m, 2 m en el
   saque de banda. Correr la envolvente después: baja el sesgo, no solo el test.
2. **Portero que ataja.** Los tiros a puerta son el 100 %.
3. **Faltas, ventaja y disciplina**, como sistema propio en
   `BallTouchSet::Contest`, que ya existe para esto.
4. **Cambio de mitades** (Ley 8), pendiente desde MVP 1.5.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Sin cambio de mitades** (Ley 8) ni tiempo añadido (Ley 7). `TeamSide` ya
  existe en el dominio, sin usar, esperando lo primero.
- **Kick-off es reanudación nominal**, no regla con posiciones y balón en juego.
- **El fuera de juego no se pita nunca**: un anotado no puede disputar el balón
  (`select_ball_challenger` lo salta), así que la regla es un campo de fuerza y
  el silbato de `referee_offside_system` no suena.
- `Ball.predictions` y `PlayerReading` son omniscientes: hoy todos leen el
  futuro real, y en MVP 4 deja de ser válido.
- `SimulationSet` refleja el orden del original; el pipeline semántico de
  `ARCHITECTURE.md` sigue pendiente. Árbitro parcial: sin faltas ni disciplina.
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
