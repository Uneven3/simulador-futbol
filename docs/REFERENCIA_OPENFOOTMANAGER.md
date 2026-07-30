# OpenFootManager como referencia — 2026-07-30

Revisión de <https://github.com/openfootmanager/openfootmanager> (GPLv3, Rust +
Tauri + React) buscando ideas aplicables. Se leyó el crate `engine`, la
herramienta `sim-bench`, sus tests y `docs/MATCH_SIMULATION.md`.

Las revisiones se fechan y no se editan.

## Qué es, y por qué su motor no nos sirve

OFM es un **manager**: el partido es abstracto. Cinco zonas lógicas
(`HomeBox`, `HomeDefense`, `Midfield`, `AwayDefense`, `AwayBox`), de una a tres
acciones por minuto resueltas por comparación de ratings, y un partido completo
en ~2 ms. No hay espacio continuo, cuerpos ni física.

Es la abstracción correcta para lo que hacen y la equivocada para lo que
hacemos: nuestro producto responde "¿qué espacio debía proteger este jugador?",
y eso exige posición continua. **Su modelo de simulación no es transferible.**

Lo que sí es transferible es su **instrumental de calibración y validación**,
que es justo lo que a nosotros nos falta.

## Lo que tomamos

### 1. Los parámetros de calibración como dato explícito

Tienen un `MatchConfig` con todo lo que decide el resultado, documentado y
sobreescribible sin recompilar:

```rust
home_advantage: 1.08,        shot_accuracy_base: 0.35,
goal_conversion_base: 0.36,  fatigue_per_minute: 0.20,
foul_probability: 0.40,      yellow_card_probability: 0.11,
red_card_probability: 0.04,  penalty_probability: 0.50,
stoppage_time_max: 4,        injury_probability: 0.03,
```

Nosotros tenemos las mismas decisiones **enterradas como literales** dentro de
`player_decisions.rs`: el umbral de tiro, el `possession_amount > 0.99`, los
24 m de amenaza, los cooldowns de toque. Están calibradas contra nada y no se
pueden barrer sin editar código.

Extraerlas a un `MatchTuning` versionado —como ya hicimos con
`MatchRegulations`— es lo que desbloquea el problema de los 51 goles/90 min:
hoy no se puede ni intentar calibrar porque no hay nada que girar.

### 2. La envolvente como herramienta, no como test ignorado

`sim-bench` es un binario que corre N partidos y reporta distribuciones:
marcadores (heatmap), goles por tramo de 15 minutos, **histograma de goles por
partido**, tiros, tiros a puerta, bloqueados, córners, tarjetas, faltas,
posesión, y un modo `--phase-sweep` que barre cada dial táctico y tabula su
efecto.

Nuestro `seeded_envelope` (diez semillas, cuatro métricas, test `#[ignore]`) es
la versión de juguete de eso. Lo que más falta es el histograma: la referencia
real son ~1,35 goles por equipo y partido, casi una Poisson, y **comparar
histogramas dice mucho más que comparar medias**.

### 3. Tests de propiedad causal

El hallazgo más valioso. No afirman valores, afirman **dirección de efecto
sobre N corridas**:

- `strong_team_wins_more_often` — 100 partidos, el fuerte gana más del doble.
- `equal_teams_roughly_even` — 200 partidos, la diferencia no llega a un tercio.
- `home_advantage_helps` — compara con y sin, misma semilla.
- `possession_style_has_more_possession` — el estilo se nota en la estadística.

Nuestro norte es literalmente "comparar decisiones tácticas" y **no tenemos ni
uno de estos**. Validamos reglas IFAB (dónde bota la pelota) pero nada que diga
"subir la línea aumenta los fueras de juego" o "presionar alto sube las
recuperaciones en campo rival".

Y son inmunes al caos que nos acaba de morder con el reloj: un test que afirma
una dirección sobre cien corridas sobrevive a una perturbación de 1 ms; uno que
afirma 1-0, no.

### 4. Un test de realismo con banda ancha

`average_goals_realistic`: 500 partidos, media entre 0,5 y 8,0 goles. Si
nosotros tuviéramos ese test llevaría meses en rojo — marcamos 51 por 90
minutos. Ponerlo, aunque nazca fallando, fija el problema en el código y no solo
en un documento que se puede ignorar.

## Lo que no tomamos, y por qué

- **El modelo de zonas y acciones por minuto.** Ver arriba.
- **`PlayStyle` como enum de seis estilos** (`Balanced`, `Attacking`,
  `Possession`, `Counter`, `HighPress`…). Es un atajo de manager: el estilo se
  elige de una lista. Nuestro objetivo es que emerja de instrucciones,
  responsabilidades y atributos (MVP 5).
- **Su separación engine ↔ domain con tipos espejo duplicados.** La hacen a
  propósito, para evolucionar el motor aislado. Nosotros acabamos de pagar por
  lo contrario: un dominio compartido es lo que impide que presentación y kernel
  diverjan, y es lo que permitió que el HUD lea el snapshot sin ver el kernel.
  Divergencia consciente.
- **Toda la capa de manager**: contratos, transferencias, moral, prensa. Otro
  producto.

## Advertencia: no comprar la idea de más de lo que es

**OFM tampoco está calibrado.** `goal_conversion_base: 0.36` es un número
elegido a mano, y una banda de "entre 0,5 y 8 goles" acepta casi cualquier cosa.
Tienen el **instrumental**, no la calibración.

Síntoma de ello: el `--help` de su CLI documenta `shot_accuracy_base` con
default 0.45 mientras `MatchConfig::default()` usa 0.35. Es el clásico knob que
vive en dos sitios — exactamente lo que hay que evitar al copiar la idea. En
nuestra versión, el valor por defecto debe existir **una sola vez** y la
herramienta debe leerlo de ahí.

## Qué cambia en nuestro plan

La calibración estaba dentro de MVP 3 ("calibrar envolventes con datos"). Pasa a
ser trabajo inmediato como **MVP 1.75**, y los tests de propiedad causal entran
como criterio de terminado. El motivo está en `AHORA.md`: un simulador a un
orden de magnitud de la única referencia externa trivial de conseguir no puede
demostrar de forma convincente ninguna regla intermedia.
