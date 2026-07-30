# Ahora

## Objetivo activo

Completar **MVP 0** y preparar el corte mínimo de **MVP 1 — Kernel observable**.

## Hecho

- Norte separado del port.
- Leyes arquitectónicas.
- Vocabulario inicial.
- Inventario IFAB 2026/27.
- Estrategia de validación.
- Port reclasificado como referencia.
- CodeGraph evaluado; instalación pendiente de un piloto A/B autorizado.

## Siguiente corte

1. Crear fronteras `domain`, `simulation`, `presentation`, `app`, idealmente
   mediante workspace/crates.
2. Extraer el spawn visual de `simulation/player_movement.rs`.
3. Separar pelota autoritativa de esfera visual.
4. Crear `VisualOf` y sincronización/interpolación de primitivas.
5. Crear `ScenarioRunner` para la misma situación headless/renderizada.
6. Definir esquema de escenarios y portar primero gol/fuera/reanudación.
7. Hacer que tests headless no registren assets.

## Deuda observada

- `PlayerMovementPlugin` crea meshes, materiales y `Visibility`.
- `RenderSetupPlugin` crea a la vez pelota autoritativa y mesh.
- `Player` mezcla identidad, rol, historial, marca y promedio de movimiento.
- `PlayerRole` mezcla posición y mentalidad.
- `Ball.predictions` es verdad compartida, no creencia individual.
- `SimulationSet` refleja el original, no el nuevo modelo.
- `eliza`, `team_ai`, `mind_set` y comentarios “port of” dominan APIs.
- Árbitro parcial; tests headless inicializan assets por bleed visual.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
