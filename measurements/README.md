# Mediciones

Aquí van las cifras que produce el simulador, y **solo aquí** (ley 15 de
`docs/ARCHITECTURE.md`). Ningún documento ni comentario copia un número de este
directorio: quedaría obsoleto al commit siguiente.

- `envelope.csv` — una fila por partido de cada corrida de `seeded_envelope`,
  con la marca de tiempo y el `sha` que la produjo. Se anexa, nunca se reescribe.
- `probes.csv` — lo que mide cada sonda, en formato largo
  (`corrida,sha,sonda,metrica,valor`): cada sonda mide cosas distintas, y una
  columna por métrica daría un archivo que es casi todo huecos.

Correr y registrar:

```
cargo test --release -p gameplayfootball_simulation seeded_envelope -- --ignored --nocapture
cargo run -p gameplayfootball_simulation --bin probe
cargo run -p gameplayfootball_simulation --bin probe -- carrying
```

La envolvente imprime el delta contra la corrida anterior: eso es lo que hay que
leer tras tocar el kernel. Una sonda no compara nada, responde una pregunta.

El código está en `crates/simulation/src/measurements.rs`.
