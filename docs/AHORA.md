# Ahora

## Objetivo activo

Completar **MVP 1 — Kernel observable**: la misma situación debe correr headless
y renderizada, con entidades autoritativas separadas de sus representaciones.

## Hecho

- Norte separado del port.
- Leyes arquitectónicas.
- Vocabulario inicial.
- Inventario IFAB 2026/27.
- Estrategia de validación.
- Port reclasificado como referencia.
- CodeGraph evaluado; instalación pendiente de un piloto A/B autorizado.
- Estado espacial de dominio: `Position` (metros, Z-up), `Facing` y `Velocity`
  en `data/spatial.rs`. `Transform` ya no es verdad en ninguna parte.
- Setup autoritativo del partido en `simulation/match_setup.rs`: pelota y dos
  onces sin meshes, materiales ni `Visibility`.
- Presentación como consumidor: `presentation/visuals.rs` crea una entidad
  desechable por cuerpo con `VisualOf`, e interpola entre las dos últimas
  posiciones del tick fijo. Cámara y luces separadas.
- Tests headless sin assets, y `simulation_runs_without_render_assets` deja la
  ley 1 como test ejecutable.

## Siguiente corte

1. Crear fronteras `domain`, `simulation`, `presentation`, `app` como crates del
   workspace, para que Cargo impida dependencias inversas.
2. Crear `ScenarioRunner` para la misma situación headless/renderizada.
3. Definir esquema de escenarios y portar primero gol/fuera/reanudación.
4. Overlays diagnósticos sobre las primitivas: predicción de la pelota,
   designación de posesión, línea de offside.

## Deuda observada

- `Player` mezcla identidad, rol, historial, marca y promedio de movimiento.
- `PlayerRole` mezcla posición y mentalidad.
- `Ball.predictions` es verdad compartida, no creencia individual.
- `SimulationSet` refleja el original, no el pipeline de `ARCHITECTURE.md`.
- `eliza`, `team_ai`, `mind_set` y comentarios "port of" dominan APIs.
- Árbitro parcial.
- `cargo clippy -- -D warnings` falla con 19 lints heredados del port
  (`collapsible_if`, `needless_range_loop`, `too_many_arguments`); ninguno fue
  introducido por el corte de capas.
- `PLAYER_HEIGHT` y el radio del cuerpo son constantes, no datos por jugador.

## Restricciones

- No mejorar IA heredada antes de separar capas.
- No añadir skinned meshes antes de primitivas desacopladas.
- No borrar algoritmos útiles del port.
- No llamar completo a un rule set sin matriz y escenarios.
