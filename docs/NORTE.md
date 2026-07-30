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

El objetivo visual es low-poly 3D con meshes, skeletons y materiales simples.
Los assets son intercambiables.

La primera presentación usará primitivas para mostrar:

- cuerpo, orientación y velocidad;
- pelota y futuros físicos;
- campo visual, observaciones y edad de memoria;
- intención y acción comprometida;
- responsabilidades tácticas;
- incidentes y decisiones arbitrales.

## Hito I — contrato completo del fútbol

Termina cuando:

- IFAB 2026/27 está inventariado y versionado;
- cada cláusula relevante tiene estado de cobertura;
- reglas deterministas tienen escenarios ejecutables;
- juicios subjetivos separan incidente, observación y decisión;
- variantes de competición son configuración;
- el kernel corre sin renderer/assets;
- primitivas permiten observar y depurar escenarios.

“Completo” significa que ninguna regla queda desconocida. La implementación se
entrega mediante MVP verticales.

## MVP

### MVP 0 — Constitución y ontología

Documentos core, vocabulario, fronteras, catálogo IFAB y formato de escenarios.
El port queda como referencia histórica.

### MVP 1 — Kernel observable

Separar entidades autoritativas de visuales. Ejecutar la misma situación
headless y con primitivas. Campo, balón, equipos, reloj, fases, gol/fuera y
reanudaciones básicas deben ser inspeccionables.

### MVP 2 — Partido reglamentariamente completo

Sustituciones, offside, faltas, ventaja, disciplina, tiros libres, penales,
dropped ball, tanda y variantes configurables. Cada transición nace de un
escenario.

### MVP 3 — Movimiento plausible

Aceleración, frenado, giro, orientación, fatiga, alcance, colisiones y
compromiso temporal. Calibrar envolventes con datos/literatura.

### MVP 4 — Percepción y creencias

Campo visual, atención, oclusión, retardo, ruido, memoria e incertidumbre. La
verdad del mundo deja de ser entrada válida para decisiones.

### MVP 5 — Responsabilidad táctica

Posiciones, roles, familiaridad, ocupación, coberturas, apoyos, presión y
políticas configurables.

### MVP 6 — Enseñanza contrafactual

Importar/construir una situación, proponer movimientos, simular alternativas y
explicar consecuencias mediante overlays y métricas.

### MVP 7 — Presentación low-poly

Skinned meshes reemplazan primitivas sin cambiar resultados; se conservan los
overlays diagnósticos.

