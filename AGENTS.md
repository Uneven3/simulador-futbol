# AGENTS.md

**Gameplay Football** es un simulador causal de fútbol en Rust + Bevy. Su
producto principal no es un port ni un juego: es una plataforma para reproducir
situaciones, comparar decisiones y enseñar movimientos bajo un modelo táctico
explícito.

Gameplay Football C++ y Google Research Football son referencias técnicas, no
la especificación del producto.

## Lectura obligatoria

1. `docs/NORTE.md`: producto y MVP incrementales.
2. `docs/ARCHITECTURE.md`: leyes y dependencias permitidas.
3. `docs/AHORA.md`: único trabajo activo.
4. `docs/DOMAIN_MODEL.md`: vocabulario canónico.
5. `docs/LAWS_OF_FOOTBALL.md`: contrato IFAB y cobertura.
6. `docs/VALIDATION.md`: cómo se demuestra fidelidad.
7. `docs/DIAGNOSTICS.md`: contrato de logs y observación.

`docs/REVISION_2026-07-30.md` es la revisión del cierre de MVP 1: qué se
verificó, qué no y qué se encontró. Las revisiones se fechan y no se editan.

La documentación histórica vive en `docs/references/gameplay_football/`.
Leer `docs/TOOLING.md` solo al cambiar herramientas de desarrollo/agentes.

## Comandos

- `cargo check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo test long_match_stats -- --ignored --nocapture`: 10 minutos simulados
  con forense de posesión; es la métrica de gameplay, no un test de regresión.

El proyecto son cuatro crates: `crates/domain`, `crates/simulation`,
`crates/presentation` y el paquete raíz como capa app (`src/`, `tests/`).
Correr cargo desde el directorio del proyecto.

La ventana se arranca con `env -u WAYLAND_DISPLAY ./target/debug/gameplayfootball`:
bajo Wayland el compositor no entrega frames a un proceso lanzado desde un shell
de agente y la app queda muda, lo que parece una regresión y no lo es.

## Leyes de trabajo

- La simulación es autoritativa y funciona headless. No depende de meshes,
  materiales, cámaras, animaciones, audio ni rutas de assets.
- Presentación crea representaciones desechables enlazadas a entidades de
  simulación. Lee snapshots y hechos; nunca decide reglas ni corrige estado.
- Components, Resources y Messages son datos. La lógica vive en sistemas y
  funciones puras.
- Cada estado mutable tiene un dueño. Otros dominios solicitan cambios mediante
  intents, hechos o mensajes tipados.
- Se distingue: verdad física → observación → creencia → intención → ejecución.
- Se distingue: incidente físico → observación arbitral → juicio → transición.
- La edición IFAB y variantes de competición son datos versionados.
- Un comportamiento no se declara realista sin métrica y referencia.
- Toda aleatoriedad de simulación usa semillas reproducibles.
- APIs nuevas no heredan nombres del original (`Eliza`, clases C++ o `AI_`).
- Usar nombres completos, newtypes y unidades explícitas; evitar `GK`, `pos`,
  `vel` o índices numéricos sin tipo en APIs.
- Sin `unsafe`. `unwrap()`/`expect()` solo para bugs de programador y tests.
- No añadir dependencias sin justificar el contrato que resuelven.
- Preservar trabajo ajeno y archivos no relacionados.

## Flujo de una feature

1. Definir fenómeno real y fuente.
2. Ubicarlo en el modelo de dominio y regla IFAB si corresponde.
3. Escribir escenarios y criterios de aceptación.
4. Implementar dato autoritativo y sistema headless.
5. Añadir presentación diagnóstica como consumidor independiente.
6. Medir contra invariantes, escenarios y datos reales.
7. Actualizar `AHORA.md`; el historial queda en Git.
