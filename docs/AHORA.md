# Ahora

Qué se está haciendo, no qué se hizo (ley 15): la historia está en `git log` y
las cifras en `measurements/`. **Techo: 40 líneas hasta la deuda declarada.**

## Objetivo activo

**MVP 4 — Que decidan con lo que ven.** Puestas las cuatro piezas del orden
acordado —forma del dato, oclusión, latencia y corte decisión/contacto— y el
cuello, que es lo que las hace no mentir: la verdad del mundo ya no entra en
ninguna decisión de movimiento.

**Cómo se valida ahora:** mirando, y con los tests de función pura, que cuestan
cero. La envolvente sigue suspendida como rutina —alarma, no diagnóstico—, pero
se corre en las dos puntas de un bloque: así el delta es atribuible en vez de
sospecha (`measurements/envelope.csv`, `a5d1489` contra `d0a6122`).

## Empezar aquí

1. **Ver mal quitó goles y robos**: 17,1 → 12,6 goles/90, 37 → 14 tiros/90 y 55
   → 15 robos/90. Quita tiros por las dos puntas —el que ataca falla y el que
   defiende no llega— y con un número no se sabe cuál pesa.
2. **Tapar es todo o nada**, y en el campo se ve medio cuerpo por encima de un
   hombro: debería subir el `blur`, no borrar. Es el candidato que queda para
   los robos, ahora que el cuello está puesto.
3. **Sensores que el fútbol tiene y esto no**: el oído —el grito es información
   sin línea de visión, justo lo que compensa la oclusión—, la altura (se
   percibe todo en planta), la identidad de lo que se ve, y el propio cuerpo:
   nadie se equivoca al calcular si llega.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Demasiados tiros son gol** (real, uno de cada diez), y de ahí un ritmo de
  gol que no puede presentarse como predicción. No es que el atacante llegue
  solo —el rival está a 2,2 m y eso nunca ha cambiado—: es lo que hace.
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

## Atributos de jugador

**Uno entra cuando tiene mecanismo que lo lee, unidad real y referencia que lo
calibra.** Están los motores y los perceptivos —campo visual, atención, reacción
y cuello—, iguales para los veintidós y con dónde diferenciarse; faltan los
tácticos de MVP 5.

## Restricciones

- No mejorar IA heredada antes de separar capas, ni añadir skinned meshes antes
  de primitivas desacopladas.
- No borrar algoritmos útiles del port, ni llamar completo un rule set sin
  matriz y escenarios.
- No añadir un atributo de jugador sin mecanismo, unidad y referencia.
- No añadir un canal de diagnóstico encendido por defecto.
