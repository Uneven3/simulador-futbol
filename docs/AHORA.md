# Ahora

Qué se está haciendo, no qué se hizo (ley 15): la historia está en `git log` y
las cifras en `measurements/`. **Techo: 40 líneas hasta la deuda declarada.**

## Objetivo activo

**MVP 4 — Que decidan con lo que ven.** Puestos todos los sensores: cono sobre
cuello, oclusión con penumbra y altura, latencia, oído —balón y «¡hombre!»—,
error de posición, de velocidad ajena y de uno mismo, y el corte
decisión/contacto. La verdad del mundo no entra en ninguna decisión.

**Cómo se valida ahora:** mirando, y con los tests de función pura, que cuestan
cero. La envolvente sigue suspendida como rutina —alarma, no diagnóstico—, pero
se corre en las dos puntas de un bloque, y así el delta es atribuible: cada sha
comparable está en `measurements/envelope.csv`.

## Empezar aquí

1. **Los sensores son iguales para los veintidós**, con el mapa ya acordado:
   `Judgement` primero —quita los hashes de sesgo—, `Senses` después, con los
   valores desde la semilla del escenario y **en espejo por dorsal**, para que
   los equipos sigan siendo iguales y la simetría aguante como invariante.
2. **La identidad no se observa**: al ver a alguien se sabe quién es, su rol y
   su casilla, porque salen del registro y no de la observación.
3. **El ritmo de gol es lo único que no se mueve con la percepción** (9,9 por 90
   contra los 2,7 reales, desde 17,1 antes de MVP 4). Es la deuda de abajo y
   espera a que se pueda calibrar, que es un hito propio.

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
- Faltan overlays de responsabilidad táctica más allá de la designación (MVP 5),
  y los canales `Formation` y `Performance` casi no tienen productores:
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
