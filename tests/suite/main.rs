//! Todo lo que se afirma sobre el partido, en un solo ejecutable.
//!
//! Cada archivo suelto en `tests/` es un binario propio para cargo, y cada
//! binario enlaza su copia de Bevy: catorce archivos eran catorce copias, y la
//! mayor parte del tiempo de `cargo test` se iba en escribirlas a disco. Aquí
//! son módulos de un target único, y lo que se corre no cambia: los nombres de
//! los tests solo ganan el prefijo del módulo.

mod documentation_budget;
mod domain_identity;
mod layer_boundaries;
mod scenarios;
