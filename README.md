# Simulador de fútbol

Simulador **causal** de fútbol asociación, escrito en Rust sobre [Bevy](https://bevyengine.org/).
No es un videojuego ni el port de uno: es una plataforma para reproducir
situaciones, comparar decisiones alternativas y explicar por qué ocurre lo que
ocurre en el campo.

Las preguntas que quiere responder:

- ¿Qué espacio debía proteger este jugador?
- ¿Qué información podía tener?
- ¿Qué carreras eran físicamente alcanzables?
- ¿Cómo cambia la respuesta bajo otro rol o modelo táctico?
- ¿Qué riesgos crea cada alternativa?

El norte completo, con sus ocho principios y los MVP en los que se entrega,
está en [`docs/NORTE.md`](docs/NORTE.md).

![Once contra once con overlays diagnósticos](docs/images/primera-verificacion-visual-2026-07-30.png)

## Estado actual: no lo uses como predicción

El proyecto está en **MVP 1.75 — calibración**. MVP 0, 1 y 1.5 están cerrados: hay
kernel autoritativo, escenarios reproducibles, árbitro parcial, reloj, diagnóstico
tipado y presentación con primitivas.

Lo que todavía no está: **el ritmo de gol es irreal**. El modelo marca unos
51 goles cada 90 minutos contra ~2,7 de un partido real
([`docs/REVISION_2026-07-30-reloj.md`](docs/REVISION_2026-07-30-reloj.md)).
Hasta calibrarlo, ningún resultado de este simulador puede presentarse como
predicción de nada. Ese es exactamente el trabajo activo, descrito en
[`docs/AHORA.md`](docs/AHORA.md), junto con el resto de deuda declarada
(sin cambio de campo, sin faltas, sin percepción parcial).

## Cómo se ejecuta

Requiere Rust estable ≥ 1.97 (edición 2024) y las dependencias de sistema de
Bevy para tu plataforma.

```bash
cargo run                 # el partido con ventana
cargo test                # la suite completa
cargo clippy --all-targets --all-features -- -D warnings
```

Con la ventana abierta, `F1` abre el hub de depuración: overlays y canales de
log, todos apagados por defecto. Sin ventana, la misma situación corre headless
con `ScenarioRunner::headless` y se observa por hechos, ledger y campo en ASCII
([`docs/DIAGNOSTICS.md`](docs/DIAGNOSTICS.md)).

Las dos formas de correr un escenario montan el mismo kernel: cualquier
divergencia entre ellas es un bug de presentación, no una diferencia de
configuración.

### Medir, no mirar

La simulación es determinista pero caótica: una sola corrida es una
trayectoria, no una métrica. La comparación válida entre dos builds es la
envolvente sobre semillas:

```bash
cargo test --release -p gameplayfootball_simulation seeded_envelope -- --ignored --nocapture
```

Diez semillas de diez minutos, reportadas como tasas. Para mirar una corrida en
detalle (pérdidas por tipo de suelta, campo en ASCII) está
`long_match_stats`, con las mismas banderas.

### Fuera del workspace `uneven`

El proyecto se desarrolla como miembro de un workspace Cargo mayor, que no
viaja en este repositorio: varios juegos comparten allí un `Cargo.lock` y los
artefactos de compilación, que en Bevy pesan demasiado para duplicarlos.

Los cuatro manifiestos no heredan nada de ese workspace, así que un clon
compila y arranca tal cual (`cargo run`). Lo que le falta es ser workspace de
sí mismo: sin eso `crates/*` no son miembros, `cargo test` sólo alcanza los
tests del paquete raíz y `cargo test -p gameplayfootball_simulation ...` no
encuentra el paquete. Se arregla añadiendo esto al final de `Cargo.toml`:

```toml
[workspace]
members = ["crates/domain", "crates/presentation", "crates/simulation"]

# Bevy es intratable en debug sin optimizar las dependencias.
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

Ese bloque es exclusivo del clon independiente: dentro del workspace `uneven`,
Cargo rechaza que un miembro sea a la vez raíz de otro workspace. Por eso no
está commiteado.

### En Linux con Wayland

El binario se arranca forzando XWayland cuando el compositor da problemas:

```bash
env -u WAYLAND_DISPLAY ./target/debug/gameplayfootball
```

## Estructura

Cuatro capas, y las fronteras las impone Cargo, no la revisión de código:

| Crate | Qué es | Qué no puede ver |
|---|---|---|
| `crates/domain` | Tipos, unidades, reglas, hechos, intenciones y configuración | Render, ventanas, assets |
| `crates/simulation` | El kernel autoritativo: física, decisión, táctica, arbitraje y telemetría | `bevy` completo — sólo subcrates de ECS, maths, tiempo y log |
| `crates/presentation` | Visuales, cámara, UI, overlays | La simulación: consume estado y hechos, nunca sistemas ni reglas |
| `src/` | La app: composición, escenarios y ciclo de vida | — |

Borrar `crates/presentation` deja un partido completo, jugable por máquina.
Las leyes y las dependencias permitidas están en
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Documentación

Toda en `docs/`, y se lee en este orden:

1. [`NORTE.md`](docs/NORTE.md) — producto y MVP incrementales.
2. [`ARCHITECTURE.md`](docs/ARCHITECTURE.md) — leyes y dependencias permitidas.
3. [`AHORA.md`](docs/AHORA.md) — el único trabajo activo.
4. [`DOMAIN_MODEL.md`](docs/DOMAIN_MODEL.md) — vocabulario canónico.
5. [`LAWS_OF_FOOTBALL.md`](docs/LAWS_OF_FOOTBALL.md) — contrato IFAB y cobertura.
6. [`VALIDATION.md`](docs/VALIDATION.md) — cómo se demuestra fidelidad.
7. [`DIAGNOSTICS.md`](docs/DIAGNOSTICS.md) — contrato de logs y observación.

Las revisiones se fechan y no se editan. `docs/references/gameplay_football/`
guarda la documentación histórica del port.

[`AGENTS.md`](AGENTS.md) es la misma información condensada para agentes de
código.

## Linaje

El proyecto empezó como port de [Gameplay Football](https://github.com/BazkieBumpercar/GameplayFootball)
(C++), y ese código sigue siendo referencia técnica —igual que Google Research
Football—, pero no la especificación del producto. Ninguna decisión de diseño se
justifica hoy diciendo que el original lo hacía así.

## Licencia

GNU General Public License v3.0 o posterior. El texto completo está en
[`LICENSE`](LICENSE).

Este programa se distribuye con la esperanza de que sea útil, pero SIN NINGUNA
GARANTÍA; ni siquiera la garantía implícita de COMERCIABILIDAD o APTITUD PARA UN
PROPÓSITO PARTICULAR.
