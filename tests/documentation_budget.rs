//! La documentación tiene presupuesto, y lo cobra un test.
//!
//! Un techo escrito dentro de un documento no se aplica solo: es exactamente la
//! clase de regla que se incumple sin que nadie lo note. Aquí falla el build.

use std::fs::read_to_string;
use std::path::{Path, PathBuf};

/// Techo total de prosa del repositorio, en líneas.
///
/// No es estética. La documentación se lee al empezar cada sesión, así que su
/// tamaño es el precio de entrada a cualquier trabajo; y un documento largo se
/// llena de cosas que pertenecen a `git log` o a `measurements/` (ley 27).
const BUDGET: usize = 1000;

/// Y `AHORA.md` tiene el suyo propio hasta la deuda declarada, porque es el que
/// se degrada primero: describe el presente y tiende a convertirse en diario.
const AHORA_BUDGET: usize = 40;

/// La documentación histórica del port no cuenta: es un archivo cerrado que no
/// se lee para trabajar, y recortarla sería reescribir la referencia.
fn prose_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![
        root.join("README.md"),
        root.join("AGENTS.md"),
        root.join("measurements/README.md"),
    ];
    let docs = root.join("docs");
    let mut listed: Vec<PathBuf> = std::fs::read_dir(&docs)
        .expect("no hay docs/")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|e| e == "md"))
        .collect();
    listed.sort();
    files.append(&mut listed);
    files
}

fn lines_of(path: &Path) -> usize {
    read_to_string(path).map(|s| s.lines().count()).unwrap_or(0)
}

#[test]
fn the_documentation_stays_within_its_budget() {
    let files = prose_files();
    let total: usize = files.iter().map(|p| lines_of(p)).sum();

    if total > BUDGET {
        let mut worst: Vec<(usize, &Path)> =
            files.iter().map(|p| (lines_of(p), p.as_path())).collect();
        worst.sort_by_key(|(lines, _)| std::cmp::Reverse(*lines));
        let biggest: Vec<String> = worst
            .iter()
            .take(3)
            .map(|(n, p)| format!("{} ({n})", p.file_name().unwrap().to_string_lossy()))
            .collect();
        panic!(
            "la documentación son {total} líneas y el techo es {BUDGET}. \
             Los tres más gordos: {}. Antes de recortar, pregúntate si lo que \
             sobra no es historia (va a `git log`) o una cifra (va a \
             `measurements/`).",
            biggest.join(", ")
        );
    }
}

#[test]
fn ahora_describes_the_present_and_not_the_past() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/AHORA.md");
    let text = read_to_string(&path).expect("no hay docs/AHORA.md");
    let before_debt = text
        .split("## Deuda declarada")
        .next()
        .expect("AHORA.md sin sección de deuda declarada");
    let lines = before_debt.lines().count();

    assert!(
        lines <= AHORA_BUDGET,
        "AHORA.md tiene {lines} líneas antes de la deuda declarada y el techo es \
         {AHORA_BUDGET}: se está convirtiendo en un diario. Lo que se hizo va al \
         mensaje de commit."
    );
}
