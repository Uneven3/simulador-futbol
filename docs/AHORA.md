# Ahora

Estado del trabajo activo. **Techo: 40 líneas hasta la deuda declarada.** Lo que
se hizo y por qué está en `git log`; las cifras están en `measurements/`. Este
archivo dice qué se está haciendo, no qué se hizo (ley 27).

## Objetivo activo

**MVP 2 — Partido reglamentariamente completo.** Sustituciones, faltas,
ventaja, disciplina, tiros libres, penales, dropped ball y tanda
(`NORTE.md`). MVP 1.75 cerró el 2026-07-30.

## Siguiente, en orden

1. **La reanudación se ejecuta.** `referee_set_piece_system` recoloca a los dos
   equipos en formación base —un espejo exacto— y suelta el balón sin dárselo a
   nadie. El empate lo rompen tres desempates que van todos para el local (los
   `<=` de `designated_player_overall` y `team_tactics`, y el `<` estricto de
   `select_ball_challenger`). Por eso `two_identical_teams_finish_level` está
   roja: 7-0 sobre seis partidos. Se arregla dando el balón a quien saca.
2. **Portero que ataja.** Los tiros a puerta son el 100 %.
3. **Faltas, ventaja y disciplina**, como sistema propio en
   `BallTouchSet::Contest`.
4. **Cambio de mitades** (Ley 8), pendiente desde MVP 1.5.

## Cómo se mide

`cargo test --release -p gameplayfootball_simulation seeded_envelope -- --ignored --nocapture`
anexa la corrida a `measurements/envelope.csv` e imprime el delta contra la
anterior. Eso, y no una corrida suelta, es la comparación válida entre builds.
Ninguna cifra se copia a este archivo.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Sin cambio de mitades** (Ley 8): los equipos defienden el mismo lado los dos
  tiempos. `TeamSide` ya existe en el dominio, sin usar, esperándolo.
- **Sin tiempo añadido** (Ley 7).
- **Kick-off es reanudación nominal**, no regla con posiciones y balón en juego.
- **El fuera de juego no se pita nunca**: un jugador anotado no puede disputar
  el balón (`select_ball_challenger` lo salta), así que la regla actúa como
  campo de fuerza y el silbato de `referee_offside_system` no suena.
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
- **El ritmo de gol es irreal**, en un orden de magnitud. Ningún resultado de
  este simulador puede presentarse como predicción hasta calibrarlo, que es un
  hito propio después de MVP 4.
- **El local gana siempre**: `two_identical_teams_finish_level` está roja a
  propósito, con la causa localizada arriba.
- **Allocations por tick** (ley 14): 18 `collect()`/`vec!` entre
  `player_decisions`, `team_tactics` y `player_movement`. Importa para MVP 6,
  que correrá cientos de variantes de la misma situación.
- **Lo visual funciona pero es pobre** (`VERIFICACION_VISUAL_2026-07-30.md`): la
  cámara ve una fracción del campo, el HUD no tiene fondo, no hay meshes de
  portería y el marcador de orientación no se distingue.

## Reparto previsto de atributos de jugador

Regla de admisión: **un atributo entra cuando tiene mecanismo que lo lee,
unidad real y referencia que lo calibra.**

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
