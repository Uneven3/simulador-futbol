# Norte del simulador

## Producto

Plataforma abierta para simular fútbol asociación, reproducir situaciones y
comparar movimientos alternativos. Debe ayudar a responder:

- ¿Qué espacio debía proteger este jugador?
- ¿Qué información podía tener?
- ¿Qué carreras eran físicamente alcanzables?
- ¿Cómo cambia la respuesta bajo otro rol o modelo táctico?
- ¿Qué riesgos crea cada alternativa?

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

- un port de Gameplay Football;
- una fuente de una única decisión táctica universal;
- una simulación muscular completa cuando un modelo reducido validado basta;
- complejidad computacional confundida con realismo.

## Presentación

El objetivo visual es low-poly 3D con meshes y materiales simples, y los assets
son intercambiables. La primera presentación usa primitivas para mostrar cuerpo,
orientación y velocidad; pelota y futuros físicos; campo visual, observaciones y
edad de memoria; intención y acción comprometida; responsabilidades tácticas; e
incidentes y decisiones arbitrales.

## Hito I — contrato completo del fútbol

Termina cuando IFAB 2026/27 está inventariado y versionado con estado de
cobertura por cláusula, las reglas deterministas tienen escenarios ejecutables,
los juicios subjetivos separan incidente / observación / decisión, las variantes
de competición son configuración, el kernel corre sin renderer y las primitivas
permiten depurar escenarios. “Completo” significa que ninguna regla queda
desconocida; se entrega mediante MVP verticales.

## MVP

Los `.5` y `.75` son trabajo de consolidación: no añaden capacidades, pagan lo
que ya está construido. Se numeran así para no invalidar las referencias
cruzadas a los MVP 2-7.

### MVP 0 — Constitución y ontología

Documentos core, vocabulario, fronteras, catálogo IFAB y formato de escenarios.
El port queda como referencia histórica.

### MVP 1 — Kernel observable

Separar entidades autoritativas de visuales. Ejecutar la misma situación
headless y con primitivas. Campo, balón, equipos, reloj, fases, gol/fuera y
reanudaciones básicas deben ser inspeccionables.

### MVP 1.75 — Instrumentación y propiedades

Antes de añadir más reglas hay que poder **medir** lo que producen las que ya
existen. Cuatro pasos: los parámetros como dato versionado (`MatchTuning`), la
envolvente como herramienta (N partidos, con distribución y no media), cada
desvío atribuido a su causa, y propiedades causales que afirman dirección de
efecto sobre N corridas en vez de valores exactos.

**Lo que deliberadamente NO hace es calibrar.** Girar parámetros hasta que
salgan 2,7 goles compensaría con números la ausencia de mecanismos: no existe
el error de golpeo, nadie tiene percepción parcial, y no hay portero ni faltas
que eviten un gol. Un modelo que acierta la media por dos errores que se
cancelan es peor que uno que falla de forma legible.

### MVP 2 — Partido reglamentariamente completo

Sustituciones, offside, faltas, ventaja, disciplina, tiros libres, penales,
dropped ball, tanda y variantes configurables. Cada transición nace de un
escenario.

Puestos: fuera de juego, reanudaciones que alguien ejecuta, portero que ataja,
cambio de mitades y tiempo añadido. La falta y la ventaja están construidas y
medidas, pero el árbitro no pita: el criterio de hoy no distingue disputar de
entrar, y eso espera al motor de MVP 3.

### MVP 3 — Movimiento plausible

Aceleración, frenado, giro, orientación, fatiga, alcance, colisiones y
compromiso temporal, con los atributos motores que los alimentan. La
calibración de este modelo hereda el instrumental de MVP 1.75: envolvente
sobre semillas y propiedades causales, no una corrida de ejemplo.

### MVP 4 — Percepción y creencias

Campo visual, atención, oclusión, retardo, ruido, memoria e incertidumbre. La
verdad del mundo deja de ser entrada válida para decisiones.

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

