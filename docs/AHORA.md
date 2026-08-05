# Ahora

Qué se está haciendo, no qué se hizo (ley 15): la historia está en `git log` y
las cifras en `measurements/`. **Techo: 40 líneas hasta la deuda declarada.**

## Objetivo activo

**MVP 4 — Que decidan con lo que ven.** Puestas las cuatro piezas del orden
acordado: la forma del dato, la oclusión, la latencia y el corte
decisión/contacto. La verdad del mundo ya no entra en ninguna decisión de
movimiento. **Desarrollo en pausa aquí**; lo de abajo es por dónde se sigue.

**Cómo se valida ahora:** mirando. La envolvente de diez semillas queda
suspendida por acuerdo —no da diagnóstico, solo alarma—; los tests unitarios de
función pura siguen, que cuestan cero. Se corrió dos veces a propósito, antes y
después de MVP 4, y por eso hay delta atribuible en vez de sospecha
(`measurements/envelope.csv`, `a5d1489` contra `d0a6122`).

## Empezar aquí

1. **La percepción bajó el ritmo de gol y no lo subió**: 17,1 → 12,6 goles/90 y
   37 → 14 tiros/90, con los robos cayendo de 55 a 15 por 90. Ver mal quita
   tiros por las dos puntas y no se sabe cuál pesa: es lo primero que hay que
   separar mirando, y la sonda `defending` es el sitio.
2. **Tapar es todo o nada**, y en el campo se ve medio cuerpo por encima de un
   hombro. Un tercio de lo que cae en el cono está detrás de alguien, y eso
   debería subir el `blur` en vez de dejar de ver. Es el candidato a explicar la
   caída de robos.
3. **Nadie predice su balón**: se persigue la trayectoria real desviada por lo
   que uno cree, que es un apaño con el signo correcto. El escalón siguiente es
   que cada uno tire de su propia recta y se equivoque en la forma, no solo en
   el sitio.
4. **La defensa no acompaña al ataque.** `grip` y `turn_rate` son iguales para
   los veintidós, y ahí es donde deberían diferenciarse; lo mismo la reacción y
   la atención, que ya tienen mecanismo y siguen siendo una constante.

## Deuda declarada, no escondida

Ausencias conocidas, no descubrimientos pendientes:

- **Demasiados tiros son gol** (real, uno de cada diez), y de ahí un ritmo de
  gol que no puede presentarse como predicción. No es que el atacante llegue
  solo —el rival está a 2,2 m y eso nunca ha cambiado—: es lo que hace.
- **16 faltas por 90 minutos contra las ~22 reales.** No se toca hasta que haya
  protección del balón: el número que hoy cuadre volvería a descuadrar.
- **La simetría solo se afirma sobre la posesión**: doce partidos no dan más.
- **El fuera de juego no se pita nunca**: un anotado no puede disputar el balón,
  así que la regla es un campo de fuerza y el silbato no suena.
- **Nadie se recupera**: el descanso entre partes no repone fatiga.
- **Girarse y salir**: la regla de proteger está, pero ponerse en medio sin
  salida deja al portador clavado.
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
técnica lateral, fatiga), y también el campo visual, la atención y la reacción,
que hoy son iguales para los veintidós y ya tienen dónde diferenciarse. Faltan
los tácticos de MVP 5.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
- No añadir un atributo de jugador sin mecanismo, unidad y referencia.
- No añadir un canal de diagnóstico encendido por defecto.
