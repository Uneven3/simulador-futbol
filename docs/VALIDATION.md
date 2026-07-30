# Validación

La simulación es fiel si reproduce fenómenos observables y responde de forma
plausible a cambios controlados. Apariencia o complejidad no son evidencia.

## Niveles

1. **Dominio:** unidades, estado finito, ownership, reproducibilidad y no-bleed.
2. **Reglas:** escenarios IFAB y trace incidente→decisión→transición.
3. **Física:** pelota, aceleración, frenado, giro, alcance y contactos.
4. **Percepción:** reacción, visibilidad, distancia, memoria e incertidumbre.
5. **Colectivo:** bloques, líneas, pitch control, coberturas y transiciones.
6. **Partido:** posesiones, pérdidas, pases, tiros, goles y distribuciones.

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

## Enseñanza contrafactual

1. Fijar situación y observaciones disponibles.
2. Cambiar una decisión/responsabilidad.
3. Simular varias semillas si existe incertidumbre.
4. Comparar espacio, líneas, riesgo, esfuerzo y resultado.
5. Mostrar intervalo/confianza, no una trayectoria “óptima” aislada.

## Disciplina

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

