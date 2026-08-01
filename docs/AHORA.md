# Ahora

Qué se está haciendo, no qué se hizo (ley 15): la historia está en `git log` y
las cifras en `measurements/`. **Techo: 40 líneas hasta la deuda declarada.**

## Objetivo activo

**MVP 4 — Que decidan con lo que ven.** Los cuerpos ya tienen motor, fatiga,
giro y percepción; lo que falta es que la percepción llegue al balón y que lo
que se ve se vea con error.

**Cómo se valida ahora:** mirando. La envolvente de diez semillas queda
suspendida por acuerdo —dos minutos por corrida y no da diagnóstico, solo
alarma—; los tests unitarios de función pura siguen, que cuestan cero.

## Empezar aquí

1. **Nadie barre el campo con la vista.** Solo se ve lo que cae en el cono
   mientras se mira al balón o adonde se corre, así que la información de los
   compañeros tiene casi cuatro segundos y se queda congelada al segundo.
2. **El balón sigue siendo omnisciente donde importa**: la trayectoria que se
   persigue es la real. Sumarle el error de creencia dejó el partido sin un solo
   tiro, porque un metro impide un contacto que se decide en sesenta y cinco.
3. **El defensor está delante y no hace nada más que estar**: bloquea el tiro si
   se cruza, pero no se cruza a propósito.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Cuatro de cada diez tiros son gol** (real, uno de cada diez), y de ahí sale
  un ritmo de gol que sigue triplicando el real: nada de esto puede presentarse
  como predicción. No es que el atacante llegue solo —el rival más cercano está
  a 2,2 m y eso no ha cambiado nunca—, es lo que ese rival hace: ver el punto 2.
- **16 faltas por 90 minutos contra las ~22 reales.** El criterio pasó de
  sobrar a faltar cuando los cuerpos tuvieron inercia, y no se toca hasta que
  haya protección del balón: el número que hoy cuadre volvería a descuadrar.
- **La simetría solo se afirma sobre la posesión**: doce partidos no dan más.
- **El fuera de juego no se pita nunca**: un anotado no puede disputar el balón,
  así que la regla es un campo de fuerza y el silbato no suena.
- **Nadie se recupera nunca**: la fatiga solo baja dentro del partido, y el
  descanso entre partes no repone nada.
- **Girarse y salir**, que es lo que le falta a proteger el balón: la regla ya
  está, pero ponerse en medio sin salida deja al portador clavado.
- `SimulationSet` refleja el orden del original; el pipeline semántico de
  `ARCHITECTURE.md` sigue pendiente.
- `PLAYER_HEIGHT` es una constante que se copia a `Attributes`, no un dato por
  jugador.
- Faltan overlays de responsabilidad táctica más allá de la designación (MVP 5).
- Los canales `Formation` y `Performance` existen en el hub y casi no tienen
  productores: `Formation` sólo emite carreras de ataque, `Performance` nada.
- **Allocations por tick** (ley 12): 18 `collect()`/`vec!` entre
  `player_decisions`, `team_tactics` y `player_movement`. Importa para MVP 6.
- **Lo visual es pobre**: el HUD no tiene fondo y no hay meshes de portería.

## Reparto previsto de atributos de jugador

**Un atributo entra cuando tiene mecanismo que lo lee, unidad real y referencia
que lo calibra.** Los motores están (velocidad, aceleración, frenado, giro,
técnica lateral, fatiga) y el campo visual también; faltan los perceptivos
—latencia, atención— y los tácticos de MVP 5.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
- No añadir un atributo de jugador sin mecanismo, unidad y referencia.
- No añadir un canal de diagnóstico encendido por defecto.
