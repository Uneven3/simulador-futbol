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
`docs/CIERRE_MVP_1_5.md` cuenta qué resolvió la consolidación y por qué se
decidió cada cosa. `docs/REVISION_2026-07-30-reloj.md` es el hallazgo que fija
el trabajo actual: el modelo marca 51 goles cada 90 minutos.
`docs/REFERENCIA_OPENFOOTMANAGER.md` revisa qué instrumental de validación se
toma de ese proyecto y qué no.

La documentación histórica vive en `docs/references/gameplay_football/`.
Leer `docs/TOOLING.md` solo al cambiar herramientas de desarrollo/agentes.

## Comandos

- `cargo check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo test --release -p gameplayfootball_simulation seeded_envelope -- --ignored --nocapture`:
  diez semillas de diez minutos, reportadas como tasas. **Es la comparación
  válida entre dos builds**; una sola corrida es una trayectoria, no una
  métrica (`docs/REVISION_2026-07-30-reloj.md`). Base actual: 51 goles/90 min
  (rango 27-81) y 19,9 cambios de posesión/min (rango 18,2-22,1).
- `cargo test --release -p gameplayfootball_simulation long_match_stats -- --ignored --nocapture`:
  una corrida con el forense completo (pérdidas por tipo de suelta, campo en
  ASCII). Sirve para mirar en detalle, no para decir si algo se rompió.
- `cargo test -p gameplayfootball -p gameplayfootball_domain -p gameplayfootball_simulation -p gameplayfootball_presentation`:
  la suite del juego. `cargo test --workspace` arrastra los otros proyectos del
  workspace y tarda muchísimo.

El proyecto son cuatro crates: `crates/domain`, `crates/simulation`,
`crates/presentation` y el paquete raíz como capa app (`src/`, `tests/`).
Correr cargo desde el directorio del proyecto.

Para observar una corrida: con ventana, `F1` abre el hub de depuración
(overlays y canales de log, todo apagado por defecto); headless,
`retaining_facts` + `MatchLedger` + `render_pitch` (ver `docs/DIAGNOSTICS.md`).

La ventana se arranca con `env -u WAYLAND_DISPLAY ./target/debug/gameplayfootball`
(fuerza XWayland) y **se puede capturar sin intervención humana**:

```bash
DISPLAY=:1 xprop -root _NET_CLIENT_LIST        # → id de la ventana
DISPLAY=:1 magick import -window 0x400002 shot.png
```

Capturar el root NO sirve: XWayland es rootless y da una imagen negra; hay que
capturar la ventana por id. Y un log vacío ya no es síntoma de nada: desde el
subsistema de diagnóstico todos los canales están apagados por defecto.

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
- Un comportamiento no se declara realista sin métrica y referencia. La métrica
  es una envolvente sobre semillas, nunca una corrida: la simulación es
  determinista pero caótica.
- Los parámetros que fijan el resultado son dato versionado (`MatchTuning` en
  `crates/domain/src/tuning.rs`), no literales dentro de la lógica, y cada valor
  por defecto vive en un solo sitio.
- Una responsabilidad por sistema; ~300 líneas por archivo y ~80 por función son
  señal de dividir. Las leyes de ingeniería están numeradas en
  `ARCHITECTURE.md` (§17-§26) y el código las cita por §; lo que hoy las viola
  está medido al final de ese documento.
- Toda aleatoriedad de simulación usa semillas reproducibles.
- APIs nuevas no heredan nombres del original (`Eliza`, clases C++ o `AI_`). Los
  comentarios sí citan el original: son trazabilidad hacia
  `references/gameplay_football/`, no nombres del presente.
- Un atributo de jugador entra cuando tiene mecanismo que lo lee, unidad real y
  referencia que lo calibra.
- Ningún canal de diagnóstico se enciende por defecto.
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
