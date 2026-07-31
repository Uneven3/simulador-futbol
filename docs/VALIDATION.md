# Validación

La simulación es fiel si reproduce fenómenos observables y responde de forma
plausible a cambios controlados. Apariencia o complejidad no son evidencia.

## Niveles

1. **Dominio:** unidades, estado finito, ownership, reproducibilidad y no-bleed.
   El no-bleed se sostiene hoy en `tests/layer_boundaries.rs`: la misma
   situación, headless y con primitivas, produce posiciones idénticas tick a
   tick, y ningún cuerpo autoritativo carga mesh.
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
modelo haya cambiado. El caso que lo demostró: el reloj truncaba milisegundos y
todos los enfriamientos se comparaban contra ese contador; al arreglarlo, el
mismo escenario con la misma semilla dio otro marcador y otro ritmo de gol. El
modelo estaba apoyado en ese ruido y ninguna prueba lo notó.

Por eso **nunca se compara una corrida**, se compara la envolvente sobre varias
semillas, y a ser posible una distribución y no una media. `seeded_envelope`
hace la versión mínima de esto.

Y por eso las **propiedades causales** son la forma de test que resiste: "el
equipo fuerte gana más veces que el débil sobre cien partidos" sigue siendo
cierto tras cualquier refactor que no cambie el modelo. Un marcador exacto, no.

## Referencias externas mínimas

Antes de cualquier afirmación de plausibilidad:

- **~2,7 goles por partido**, ~1,35 por equipo, distribuidos casi como una
  Poisson. Es la referencia más barata que existe, y el modelo sigue a un orden
  de magnitud de ella: lo que mide `seeded_envelope` en cada corrida.
- Las demás (tiros, posesión, pases completados) salen de los datasets de la
  sección siguiente.

## Fuentes

- IFAB 2026/27: <https://www.theifab.com/laws/latest/>
- Metrica tracking/eventos: <https://github.com/metrica-sports/sample-data>
- SkillCorner open data: <https://github.com/SkillCorner/opendata>
- floodlight: <https://github.com/floodlight-sports/floodlight>
- kloppy: <https://kloppy.pysport.org/>

Gameplay Football y RoboCup sugieren implementaciones; no son verdad empírica.

## Escenario

Registra edición/competición, campo, estado inicial, participantes/perfiles,
plan/responsabilidades, semilla, ventana simulada, hechos esperados, métricas y
tolerancias. Cámara/overlays son opcionales y no autoritativos.

El mismo archivo alimenta runner headless, primitivas y replay.

Implementado en `crates/domain/src/scenario.rs` (`Scenario`, `Expectations`,
`ScenarioOutcome::mismatches`) y ejecutado por `ScenarioRunner` con `headless` o
`with_primitives`. Pendiente: perfiles y responsabilidades por jugador,
colocación explícita, métricas con tolerancia, y el escenario como archivo —
hoy el catálogo son datos en Rust (`src/scenarios.rs`).

## Enseñanza contrafactual

1. Fijar situación y observaciones disponibles.
2. Cambiar una decisión/responsabilidad.
3. Simular varias semillas si existe incertidumbre.
4. Comparar espacio, líneas, riesgo, esfuerzo y resultado.
5. Mostrar intervalo/confianza, no una trayectoria “óptima” aislada.

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

