//! Las sondas: preguntas medidas, no afirmaciones.
//!
//! Cada una responde con números a una pregunta sobre el partido, y ninguna
//! afirma nada; por eso no son tests. Vivían en `tests/` marcadas `#[ignore]`,
//! y cada archivo de ahí es un ejecutable propio: nueve copias de Bevy
//! enlazadas —1,7 GB cada una— que se reenlazaban en cada `cargo test` para no
//! ejecutarse nunca. Aquí son un binario y salen del ciclo de test.
//!
//! ```text
//! cargo run -p gameplayfootball_simulation --bin probe -- carrying
//! cargo run -p gameplayfootball_simulation --bin probe  # el índice
//! ```

use football_simulation::measurements;

mod carrying;
mod commitment;
mod defending;
mod foul;
mod perception;
mod shielding;
mod speed;
mod stamina;
mod symmetry;

/// Nombre, pregunta y cuerpo. La pregunta está aquí para que `probe` sin
/// argumentos sea el índice: una sonda que nadie sabe que existe no se corre.
const PROBES: &[(&str, &str, fn())] = &[
    (
        "carrying",
        "¿se orbita el balón, y se pierde sin que nadie apriete?",
        carrying::run,
    ),
    (
        "commitment",
        "¿cuántos golpeos se quedan sin dar?",
        commitment::run,
    ),
    (
        "defending",
        "¿cuánta compañía tiene el atacante cerca del área?",
        defending::run,
    ),
    (
        "foul",
        "¿cuántas faltas ve el árbitro con el criterio de hoy?",
        foul::run,
    ),
    (
        "perception",
        "¿qué sabe cada jugador del campo?",
        perception::run,
    ),
    (
        "shielding",
        "¿llega alguien a tener un cuerpo delante del balón?",
        shielding::run,
    ),
    (
        "speed",
        "¿a cuántas veces el tiempo real corre un partido?",
        speed::run,
    ),
    (
        "stamina",
        "¿cómo llegan las piernas al final?",
        stamina::run,
    ),
    ("symmetry", "¿el sesgo es de lado o de rol?", symmetry::run),
];

/// Lo medido va al CSV además de a la pantalla: leer los números ahora es una
/// cosa, y compararlos con los de la semana pasada es otra. Anexa, nunca
/// reescribe, y una escritura fallida no invalida la medición que ya se
/// imprimió.
pub fn record(probe: &str, metrics: &[(&str, f32)]) {
    let path = measurements::probe_log();
    match measurements::record(probe, metrics, &path) {
        Ok(id) => println!(
            "\n{} filas anexadas a {} ({id})",
            metrics.len(),
            path.display()
        ),
        Err(error) => eprintln!("\nno se pudo escribir {}: {error}", path.display()),
    }
}

fn main() {
    let Some(name) = std::env::args().nth(1) else {
        println!("sondas disponibles:\n");
        for (name, question, _) in PROBES {
            println!("  {name:<12} {question}");
        }
        println!("\nuso: cargo run -p gameplayfootball_simulation --bin probe -- <nombre>");
        return;
    };

    match PROBES.iter().find(|(probe, _, _)| *probe == name) {
        Some((_, question, run)) => {
            println!("== {name}: {question}\n");
            run();
        }
        None => {
            eprintln!("no hay ninguna sonda llamada '{name}'; corre el binario sin argumentos");
            std::process::exit(1);
        }
    }
}
