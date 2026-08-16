# Validación

La simulación es fiel si reproduce fenómenos observables y responde de forma
plausible a cambios controlados. Apariencia o complejidad no son evidencia.

## Niveles

1. **Dominio:** unidades, estado finito, ownership, reproducibilidad y no-bleed
   (`tests/suite/layer_boundaries.rs`: headless y con primitivas dan las mismas
   posiciones tick a tick).
2. **Reglas:** escenarios IFAB y trace incidente→decisión→transición.
3. **Física:** pelota, aceleración, frenado, giro, alcance y contactos.
4. **Percepción:** reacción, visibilidad, distancia, memoria e incertidumbre.
5. **Colectivo:** bloques, líneas, pitch control, coberturas y transiciones.
6. **Partido:** posesiones, pérdidas, pases, tiros, goles y distribuciones.
7. **Propiedades causales:** una decisión táctica produce el efecto esperado, en
   dirección y no en valor.

## Cómo se compara una corrida con otra

Una corrida es una trayectoria, no una métrica. La simulación es determinista
pero caótica: una perturbación de un milisegundo produce otro partido sin que el
modelo haya cambiado. Por eso **nunca se compara una corrida**, se compara la
envolvente sobre varias semillas (`seeded_envelope`).

Pero la envolvente **avisa, no diagnostica**. Dice que el ritmo de gol se movió,
nunca por qué, y de ella han salido dos diagnósticos falsos seguidos. Lo que
encuentra causas son otras dos cosas:

- **La sonda dirigida al defecto**: mide exactamente lo que se sospecha
  —cuántos golpeos mueren sin darse, a qué distancia está el rival más cercano,
  cuánto tiempo va el balón en el pie—. Se escribe para una pregunta y se queda
  como registro de la respuesta. No son tests, porque no afirman nada: viven en
  `cargo run -p gameplayfootball_simulation --bin probe -- <sonda>` (sin
  argumentos, el índice) y anexan lo medido a `measurements/probes.csv`.
- **Mirar el partido.** Tres defectos que ninguna métrica delató en días
  —conducir a velocidad de sprint, orbitar el balón, el sacador que se va con
  él— se vieron en cinco minutos de ventana. Un ojo humano es un instrumento, y
  el más barato que hay.

Y por eso las **propiedades causales** son la forma de test que resiste: "el
equipo fuerte gana más veces que el débil sobre cien partidos" sigue siendo
cierto tras cualquier refactor que no cambie el modelo. Un marcador exacto, no.

## Referencias externas mínimas

**~2,7 goles por partido**, ~1,35 por equipo, casi como una Poisson: la
referencia más barata que existe. Las demás —tiros, posesión, pases
completados— salen de los datasets de abajo.

## Fuentes

- IFAB 2026/27: <https://www.theifab.com/laws/latest/>
- Metrica tracking/eventos: <https://github.com/metrica-sports/sample-data>
- SkillCorner open data: <https://github.com/SkillCorner/opendata>
- floodlight: <https://github.com/floodlight-sports/floodlight>
- kloppy: <https://kloppy.pysport.org/>

Gameplay Football y RoboCup sugieren implementaciones; no son verdad empírica.

## Escenario

Registra edición/competición, campo, estado inicial, participantes, plan,
semilla, ventana, hechos esperados y tolerancias, y alimenta igual al runner
headless y al de primitivas. Implementado en `domain::scenario` y ejecutado por
`simulation::scenario_runner`: colocaciones, propuestas y observaciones
iniciales son explícitas. El RON versionado intercambia el escenario causal
completo; el runner confirma que importarlo reproduce la misma corrida.
`CounterfactualReport` compara sus envolventes con las mismas semillas.

## Enseñanza contrafactual

Fijar situación y observaciones, cambiar una decisión, simular varias semillas,
y comparar espacio, líneas, riesgo, esfuerzo y resultado mostrando el intervalo
y no una trayectoria «óptima» aislada.

## Disciplina

- los parámetros que fijan el resultado son **dato versionado**, no literales
  dentro de la lógica, y cada valor por defecto vive en un solo sitio;
- separar calibración y validación;
- versionar parámetros/datasets;
- registrar antes/después;
- no mejorar una métrica ocultando regresiones;
- distinguir error de modelo, sensor y árbitro;
- guardar bugs como escenarios mínimos.

## Presentación diagnóstica

Las primitivas son instrumento científico. Deben alternar verdad/creencia,
edad/incertidumbre, orientación/campo visual, intención/ejecución, regiones
tácticas, contactos/incidentes/decisiones e IDs/unidades. Los overlays solo leen.
