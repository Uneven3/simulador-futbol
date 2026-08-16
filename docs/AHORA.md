# Ahora

Qué se está haciendo, no qué se hizo (ley 15): la historia está en `git log` y
las cifras en `measurements/`. **Techo: 40 líneas hasta la deuda declarada.**

## Objetivo activo

**Calibración — hito propio después de MVP 4.** Los MVP 0–7 están entregados.
El trabajo activo ya no es añadir capacidades: es medir la distribución de diez
semillas contra su corrida anterior, fijar tolerancias antes de ajustar y usar
partidos completos contra Poisson sólo al calibrar.

**Cómo se valida ahora:** mirando, y con los tests de función pura, que cuestan
cero. La envolvente sigue suspendida como rutina —alarma, no diagnóstico—, pero
se corre en las dos puntas de un bloque, y así el delta es atribuible: cada sha
comparable está en `measurements/envelope.csv`.

## Empezar aquí

1. Corre `seeded_envelope` antes y después de un ajuste y anexa el resultado a
   `measurements/envelope.csv`; no compares una trayectoria sola.
2. Fija la métrica, tolerancia y referencia antes de tocar `MatchTuning`.
3. Usa `goal_distribution` para contrastar partidos completos con Poisson;
   nunca para validar un cambio rutinario.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Demasiados tiros son gol** (real, uno de cada diez), y de ahí un ritmo de gol
  que no es una predicción. No es que el atacante llegue solo —el rival está a
  2,2 m y nunca ha cambiado—: es lo que hace.
- **Las faltas se quedan cortas** contra las ~22 reales, y no se tocan hasta que
  haya protección del balón: el número que hoy cuadre volvería a descuadrar. **La
  simetría** solo se afirma sobre la posesión: doce partidos no dan más.
- **El fuera de juego no se pita nunca**: un anotado no puede disputar el balón,
  así que la regla es un campo de fuerza y el silbato no suena.
- **Nadie se recupera**: el descanso entre partes no repone fatiga. **Girarse y
  salir**: proteger está, pero ponerse en medio sin salida deja al portador
  clavado.
- `SimulationSet` refleja el orden del original; el pipeline semántico de
  `ARCHITECTURE.md` sigue pendiente. `PLAYER_HEIGHT` es una constante que se
  copia a `Attributes`, no un dato por jugador.
- Los canales `Formation` y `Performance` casi no tienen productores:
  `Formation` sólo emite carreras de ataque, `Performance` nada.
- **Allocations por tick** (ley 12): 18 `collect()`/`vec!` entre
  `player_decisions`, `team_tactics` y `player_movement`. Importa para MVP 6.
  Y **lo visual es pobre**: el HUD sin fondo, y no hay meshes de portería.

## Restricciones

- No mejorar IA heredada antes de separar capas, ni añadir skinned meshes antes
  de primitivas desacopladas.
- No borrar algoritmos útiles del port, ni llamar completo un rule set sin
  matriz y escenarios.
- No añadir un atributo de jugador sin mecanismo, unidad y referencia.
- No añadir un canal de diagnóstico encendido por defecto.
