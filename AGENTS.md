# AGENTS.md

**Gameplay Football** es un simulador causal de fútbol en Rust + Bevy. No es un
port ni un juego: es una plataforma para reproducir situaciones, comparar
decisiones y enseñar movimientos bajo un modelo táctico explícito. Gameplay
Football C++ y Google Research Football son referencia técnica, no
especificación; su documentación vive en `docs/references/gameplay_football/`.

## Empieza por

1. `docs/AHORA.md` — el trabajo activo, y solo eso.
2. `docs/ARCHITECTURE.md` — las 27 leyes, vinculantes. El código las cita por §.

El resto, cuando toque lo que cubren, no antes: `NORTE.md` (producto y MVP),
`DOMAIN_MODEL.md` (vocabulario), `LAWS_OF_FOOTBALL.md` (IFAB), `VALIDATION.md`
(cómo se demuestra fidelidad), `DIAGNOSTICS.md` (observación).

**Las cifras no viven en la documentación** (ley 27): van a `measurements/`, y
la historia al mensaje de commit. Un documento que hay que reeditar cada vez que
cambia un número está mal puesto.

## Comandos

El proyecto son cuatro crates —`crates/domain`, `crates/simulation`,
`crates/presentation` y el paquete raíz como capa app— dentro de un workspace
compartido con otros juegos. Correr cargo desde el directorio del proyecto.

- `cargo clippy --all-targets -- -D warnings` y `cargo fmt --all -- --check`.
- `cargo test -p gameplayfootball -p gameplayfootball_domain -p gameplayfootball_simulation -p gameplayfootball_presentation`
  — la suite. `--workspace` arrastra los otros proyectos y tarda muchísimo.
- `cargo test --release -p gameplayfootball_simulation seeded_envelope -- --ignored --nocapture`
  — diez semillas de diez minutos. **Es la comparación válida entre dos builds**:
  una corrida suelta es una trayectoria, no una métrica. Anexa a
  `measurements/envelope.csv` e imprime el delta contra la anterior.
- `cargo test --release -p gameplayfootball_simulation goal_distribution -- --ignored --nocapture`
  — veinte partidos completos contra la Poisson real. Dos minutos por partido:
  se corre al calibrar, no de rutina.
- `cargo test --release -p gameplayfootball_simulation --test causal_properties -- --ignored --nocapture`
  — dirección de efecto, no valores. Correr al tocar `MatchTuning` o la IA.

La herramienta detrás de las tres es `crates/simulation/src/envelope.rs`; los
tests solo la imprimen.

## Observar una corrida

Con ventana, `F1` abre el hub de depuración (overlays y canales, todo apagado
por defecto, así que un log vacío no es síntoma de nada). Headless,
`retaining_facts` + `MatchLedger` + `render_pitch` (`docs/DIAGNOSTICS.md`).

La ventana se arranca con `env -u WAYLAND_DISPLAY ./target/debug/gameplayfootball`
y se captura sin intervención humana:

```bash
DISPLAY=:1 xprop -root _NET_CLIENT_LIST        # → id de la ventana
DISPLAY=:1 magick import -window 0x400002 shot.png
```

Capturar el root da negro: XWayland es rootless, hay que ir por id de ventana.

## Flujo de una feature

Fenómeno real y fuente → sitio en el modelo de dominio y regla IFAB → escenarios
y criterios de aceptación → dato autoritativo y sistema headless → presentación
diagnóstica como consumidor → medir contra invariantes y datos reales. El
historial queda en Git; `AHORA.md` solo si cambia el objetivo activo.
