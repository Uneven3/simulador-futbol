# Norte del simulador

## Producto

Plataforma abierta para simular fútbol asociación, reproducir situaciones y
comparar movimientos alternativos: qué espacio debía proteger un jugador, qué
información podía tener, qué carreras eran alcanzables, cómo cambia la respuesta
bajo otro rol y qué riesgo crea cada alternativa.

Puede llegar a ser jugable, pero el juego será un consumidor. Diversión,
balance y espectáculo no alteran el modelo sin declararse como configuración.

## Principios

1. **Causal antes que visual.** Los resultados surgen de reglas, estado,
   percepción, decisión y capacidad motora identificables.
2. **Información situada.** Los jugadores observan parcialmente, recuerdan y
   estiman con incertidumbre.
3. **Cuerpo sin asset.** La capacidad física pertenece a datos; mesh y clip
   solo representan el resultado.
4. **Táctica explícita.** “Debería” significa “según este rol, responsabilidad,
   objetivo y tolerancia al riesgo”.
5. **Explicable.** Se exponen hechos, creencias, intenciones y alternativas.
6. **Validado.** Se compara con IFAB, tracking, eventos y distribuciones reales.
7. **Reproducible.** Escenario, configuración y semilla fijan el resultado.
8. **Presentación reemplazable.** Headless, primitivas, replay y low-poly
   comparten la misma simulación.

## No es

Un port de Gameplay Football, una fuente de la decisión táctica universal, una
simulación muscular donde basta un modelo reducido validado, ni complejidad
computacional confundida con realismo.

## Presentación

Low-poly 3D con assets intercambiables. Las primitivas de hoy muestran cuerpo,
orientación y velocidad; pelota y futuros; campo visual, observaciones y edad de
memoria; intención y acción comprometida; e incidentes arbitrales.

## Hito I — contrato completo del fútbol

Termina cuando IFAB 2026/27 está inventariado con cobertura por cláusula, las
reglas deterministas tienen escenarios ejecutables, los juicios subjetivos
separan incidente / observación / decisión, las variantes de competición son
configuración y el kernel corre sin renderer. “Completo” significa que ninguna
regla queda desconocida; se entrega mediante MVP verticales.

## MVP

Los `.5` y `.75` son trabajo de consolidación: no añaden capacidades, pagan lo
que ya está construido.

**Son ejes, no fases.** Se avanzó en espiral y con razón: las reglas de MVP 2
no significaban nada sobre cápsulas —las faltas pasaron de 168 por 90 a 16 por
mecanismos del cuerpo, sin tocar al árbitro— y la percepción de MVP 4 solo se
sostiene sobre cuerpos que miran. Terminar un eje antes de tocar el siguiente
produce números que se caen cuando el siguiente llega.

### MVP 0 — Constitución y ontología

Documentos core, vocabulario, fronteras, catálogo IFAB y formato de escenarios.
El port queda como referencia histórica.

### MVP 1 — Kernel observable

Entidades autoritativas separadas de las visuales, la misma situación headless
y con primitivas, y campo, balón, equipos, reloj, fases, gol/fuera y
reanudaciones inspeccionables.

### MVP 1.75 — Instrumentación y propiedades

Parámetros como dato versionado (`MatchTuning`), envolvente sobre semillas y
propiedades causales que afirman dirección y no valor.

**No calibra, y eso no ha cambiado.** Girar parámetros hasta que salga el ritmo
de gol real compensaría con números la ausencia de mecanismos. Para qué sirve
cada instrumento —quién avisa y quién diagnostica— está en `VALIDATION.md`.

### MVP 2 — Partido reglamentariamente completo

Sustituciones, offside, faltas, ventaja, disciplina, tiros libres, penales,
dropped ball, tanda y variantes configurables. Cada transición nace de un
escenario.

Puestos: fuera de juego, reanudaciones que alguien ejecuta y no puede
conducir, portero que ataja, cambio de mitades, tiempo añadido, y la falta con
su ventaja, ya pitada. Faltan sustituciones, disciplina, penales, dropped ball
y tanda.

### MVP 3 — Movimiento plausible

Aceleración, frenado, agarre, orientación, fatiga, alcance, colisiones y
compromiso temporal, con los atributos motores que los alimentan.

Puestos todos menos el alcance. Falta la **acción defensiva individual** —salir
a achicar el ángulo, meter la pierna—, que es motor y decisión antes que táctica
de equipo, y **jugadores que no sean clones** (`AHORA.md`).

### MVP 4 — Percepción y creencias

Campo visual, atención, oclusión, retardo, ruido, memoria e incertidumbre. La
verdad del mundo deja de ser entrada válida para decisiones.

Puestos el cono, la memoria que envejece y el detalle que decae con la
distancia; las decisiones leen creencias. Falta barrer el campo con la vista y
el balón, que sigue siendo omnisciente en la trayectoria que se persigue.

### MVP 5 — Responsabilidad táctica

Posiciones, roles, familiaridad, ocupación, coberturas, apoyos, presión y
políticas configurables.

### MVP 6 — Enseñanza contrafactual

Importar/construir una situación, proponer movimientos, simular alternativas y
explicar consecuencias mediante overlays y métricas.

### Calibración — hito propio, después de MVP 4

Cuando existan portero que ataja, faltas que interrumpen, motor con fatiga y
percepción parcial, entonces sí: girar `MatchTuning` contra la distribución
real (~1,35 goles por equipo, casi Poisson) y no contra una media. Antes de
eso, el instrumental de MVP 1.75 solo sirve para comparar builds y para afirmar
propiedades, que es lo que se le pide.

### MVP 7 — Presentación low-poly

Skinned meshes reemplazan primitivas sin cambiar resultados; se conservan los
overlays diagnósticos.

