# El reloj tenía ruido, y el modelo estaba apoyado en él — 2026-07-30

Hallazgo del último tramo de MVP 1.5, al convertir el tiempo del kernel a
`Duration` (ley 9). Es la clase de cosa que la revisión anterior no podía ver
porque medía con una sola semilla.

## Qué pasaba

Cada sistema calculaba el tiempo de partido así:

```rust
let now_ms = (time.elapsed_secs_f64() * 1000.0) as u64;
```

Con ticks de 10 ms exactos eso *debería* dar múltiplos de 10. No los da: la
conversión a `f64` y el truncamiento se comen un milisegundo en **590 de cada
60.000 ticks (≈1 %)**, y no de forma regular — el contador salta 11 ms y luego
9 ms. `2.01 s` se convierte en 2009 ms, no en 2010.

Todos los cooldowns del juego se comparaban contra ese contador: 220 ms para
recoger un balón suelto, 400 ms para el último tocador, 500/1000 ms para robar,
150/350 ms para decidir y tocar.

## Qué pasó al arreglarlo

`Duration` es exacto. Con el reloj arreglado, **el ritmo de gol se duplica**.
Medido sobre las mismas cinco semillas, diez minutos simulados cada una:

| | goles/90 min (media) | cambios de posesión/min |
|---|---|---|
| Reloj con ruido | 27 | 21,9 |
| Reloj exacto | 58 | 19,9 |

Ampliado a diez semillas con el reloj exacto: **51 goles/90 min de media (rango
27-81, sd 13)** y **19,9 cambios/min (rango 18,2-22,1, sd 1,4)**.

## Qué significa

Dos cosas, y la segunda importa más que la primera.

1. **El reloj nuevo es el correcto.** Un contador de milisegundos que a veces
   retrocede respecto al tiempo real no es una decisión de diseño, es un error.
   La conversión se queda.

2. **El modelo nunca estuvo calibrado, y ahora se ve.** Un partido real tiene
   ~2,7 goles. El modelo daba 27 por 90 minutos *antes* del arreglo: ya era diez
   veces la realidad. El reloj exacto lo llevó a 51, pero el problema no es el
   reloj — es que las envolventes de tiro, portero y defensa vienen del tuning
   del original y **no se han comparado con nada**. Que un cambio de 1 ms en el
   1 % de los ticks duplique el marcador dice además que el modelo es
   caóticamente sensible en la zona donde se decide un gol.

## Qué cambia en cómo se mide

La referencia que usábamos como "no regresión" era una corrida con una semilla:
1-0, 205 cambios de posesión, racha de 16,7 s. Eso **no era una métrica**, era
una trayectoria: cualquier perturbación la cambia sin que el modelo haya
empeorado ni mejorado.

En su lugar hay ahora `seeded_envelope` (`cargo test --release -p
gameplayfootball_simulation seeded_envelope -- --ignored --nocapture`): las
mismas diez semillas, reportadas como tasas. Se compara la envolvente, no el
partido.

`long_match_stats` sigue siendo útil para *mirar* una corrida en detalle —
forense de pérdidas, campo en ASCII — pero no para decir si algo se rompió.

## Qué queda abierto

- **Calibrar el ritmo de gol es MVP 3.** La primera referencia externa es
  trivial de conseguir (goles por partido de cualquier liga) y el modelo está a
  un orden de magnitud de ella. Sospechosos: la portería sin portero que la
  defienda de verdad, el umbral de tiro y la ausencia de faltas.
- Con 51 goles cada 90 minutos, ningún resultado de este simulador puede
  presentarse todavía como una predicción de nada. Eso ya estaba en la deuda
  declarada; ahora tiene número.
