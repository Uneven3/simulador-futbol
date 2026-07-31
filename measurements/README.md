# Mediciones

Aquí van las cifras que produce el simulador, y **solo aquí** (ley 27 de
`docs/ARCHITECTURE.md`). Ningún documento ni comentario copia un número de este
directorio: quedaría obsoleto al commit siguiente.

- `envelope.csv` — una fila por partido de cada corrida de `seeded_envelope`,
  con la marca de tiempo y el `sha` que la produjo. Se anexa, nunca se reescribe.

Correr y registrar:

```
cargo test --release -p gameplayfootball_simulation seeded_envelope -- --ignored --nocapture
```

imprime el delta contra la corrida anterior. Eso es lo que hay que leer tras
tocar el kernel; el informe completo es para cuando el delta sorprende.

El código está en `crates/simulation/src/measurements.rs`.
