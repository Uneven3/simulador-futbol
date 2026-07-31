//! Las mediciones son datos, no prosa.
//!
//! Una envolvente produce números que hay que comparar con los de ayer. Cuando
//! esos números viven en un documento, cada medición obliga a editar cuatro
//! sitios, quedan obsoletos al commit siguiente y nadie puede graficarlos.
//! Aquí se anexan a un CSV: una fila por partido, con el `sha` que los produjo.
//!
//! El formato es CSV a propósito: se anexa sin releer, se grepea, `git diff` lo
//! muestra por filas y no necesita ningún parser para leerse a ojo.

use crate::envelope::EnvelopeReport;
use football_domain::TeamId;
use std::fmt::Write as _;
use std::fs::{OpenOptions, create_dir_all, read_to_string};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;

const HEADER: &str = "corrida,sha,ventana_s,semilla,goles_local,goles_visitante,tiros,a_puerta,pases,completados,cambios_min,robos,recogidas";

/// Dónde se anexa: `measurements/envelope.csv` en la raíz del repositorio, que
/// no es el directorio desde el que corre el test.
pub fn default_log() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("measurements/envelope.csv")
}

/// Una corrida entera, tal y como quedó en el CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub id: String,
    pub sha: String,
    /// Las medias de la corrida, en el orden en que se comparan.
    pub goals_per_90: f32,
    pub shots_per_90: f32,
    pub on_target: f32,
    pub passes_per_90: f32,
    pub pass_completion: f32,
    pub changes_per_minute: f32,
    pub matches: usize,
}

/// Anexa una corrida y devuelve su identificador.
///
/// El identificador es la marca de tiempo, y el `sha` dice contra qué código se
/// midió: sin él una fila no se puede atribuir a nada.
pub fn append(report: &EnvelopeReport, path: &Path) -> io::Result<String> {
    let id = timestamp();
    let sha = head_sha();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let fresh = !path.exists();
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    if fresh {
        writeln!(file, "{HEADER}")?;
    }
    for m in &report.matches {
        writeln!(
            file,
            "{id},{sha},{:.0},{:#x},{},{},{},{},{},{},{:.2},{},{}",
            report.window.as_secs_f32(),
            m.seed,
            m.goals[TeamId::Home],
            m.goals[TeamId::Away],
            m.total_shots(),
            m.shots_on_target[TeamId::Home] + m.shots_on_target[TeamId::Away],
            m.passes[TeamId::Home] + m.passes[TeamId::Away],
            m.passes_completed[TeamId::Home] + m.passes_completed[TeamId::Away],
            m.possession_changes_per_minute,
            m.tackles,
            m.loose_balls,
        )?;
    }
    Ok(id)
}

/// Las corridas del CSV, en orden, con sus medias ya calculadas.
///
/// Una fila rota no invalida el archivo: se salta. El registro es un diario, no
/// una estructura que haya que poder cargar entera o fallar.
pub fn runs(path: &Path) -> Vec<Run> {
    let Ok(text) = read_to_string(path) else {
        return Vec::new();
    };
    let mut runs: Vec<Run> = Vec::new();
    let mut per_90: Vec<[f32; 6]> = Vec::new();

    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() != 13 {
            continue;
        }
        let num = |i: usize| f[i].parse::<f32>().unwrap_or(0.0);
        let window_s = num(2);
        if window_s <= 0.0 {
            continue;
        }
        let to_90 = 90.0 * 60.0 / window_s;
        let sample = [
            (num(4) + num(5)) * to_90,
            num(6) * to_90,
            num(7),
            num(8) * to_90,
            num(9),
            num(10),
        ];

        if runs.last().is_none_or(|r| r.id != f[0]) {
            flush(&mut runs, &mut per_90);
            runs.push(Run {
                id: f[0].to_string(),
                sha: f[1].to_string(),
                goals_per_90: 0.0,
                shots_per_90: 0.0,
                on_target: 0.0,
                passes_per_90: 0.0,
                pass_completion: 0.0,
                changes_per_minute: 0.0,
                matches: 0,
            });
        }
        // los porcentajes viajan como conteos y se dividen al final, o una media
        // de porcentajes por partido no es el porcentaje de la corrida
        per_90.push(sample);
        if let Some(run) = runs.last_mut() {
            run.matches += 1;
        }
    }
    flush(&mut runs, &mut per_90);
    runs
}

/// Cierra la corrida en curso: convierte los conteos acumulados en medias.
fn flush(runs: &mut [Run], samples: &mut Vec<[f32; 6]>) {
    let (Some(run), false) = (runs.last_mut(), samples.is_empty()) else {
        samples.clear();
        return;
    };
    let n = samples.len() as f32;
    let sum = |i: usize| samples.iter().map(|s| s[i]).sum::<f32>();
    run.goals_per_90 = sum(0) / n;
    run.shots_per_90 = sum(1) / n;
    run.on_target = if sum(1) > 0.0 { sum(2) / sum(1) } else { 0.0 };
    run.passes_per_90 = sum(3) / n;
    run.pass_completion = if sum(3) > 0.0 { sum(4) / sum(3) } else { 0.0 };
    run.changes_per_minute = sum(5) / n;
    samples.clear();
}

/// Qué cambió respecto de la corrida anterior, o por qué no hay con qué
/// comparar. Es lo único que hace falta leer tras un refactor del kernel.
pub fn compare_last_two(path: &Path) -> String {
    let runs = runs(path);
    let Some(current) = runs.last() else {
        return "sin mediciones registradas".to_string();
    };
    let Some(previous) = runs.len().checked_sub(2).map(|i| &runs[i]) else {
        return format!("primera corrida registrada ({})", current.sha);
    };

    let mut out = format!(
        "contra la corrida anterior ({} → {})\n",
        previous.sha, current.sha
    );
    let rows: [(&str, f32, f32); 6] = [
        ("goles/90", previous.goals_per_90, current.goals_per_90),
        ("tiros/90", previous.shots_per_90, current.shots_per_90),
        (
            "a puerta %",
            previous.on_target * 100.0,
            current.on_target * 100.0,
        ),
        ("pases/90", previous.passes_per_90, current.passes_per_90),
        (
            "completados %",
            previous.pass_completion * 100.0,
            current.pass_completion * 100.0,
        ),
        (
            "cambios/min",
            previous.changes_per_minute,
            current.changes_per_minute,
        ),
    ];
    for (label, before, after) in rows {
        let delta = after - before;
        let mark = if delta.abs() < 1e-3 { "=" } else { "" };
        let _ = writeln!(
            out,
            "  {label:<14} {before:>8.1} → {after:>8.1}  ({delta:+.1}){mark}"
        );
    }
    out
}

fn timestamp() -> String {
    Command::new("date")
        .arg("+%Y-%m-%dT%H:%M:%S")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "desconocida".to_string())
}

fn head_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "sin-git".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las medias de una corrida salen de sus filas, y los porcentajes se
    /// dividen sobre los totales y no promediando porcentajes.
    #[test]
    fn a_run_is_the_mean_of_its_rows() {
        let dir = std::env::temp_dir().join("gpf-measurements-test");
        let _ = create_dir_all(&dir);
        let path = dir.join("envelope.csv");
        let _ = std::fs::remove_file(&path);
        // ventana de 90 min: las tasas por 90 son los conteos tal cual
        let rows = format!(
            "{HEADER}\n\
             A,sha1,5400,0x1,1,1,10,5,100,50,20.0,2,30\n\
             A,sha1,5400,0x2,3,1,30,10,300,60,10.0,4,50\n\
             B,sha2,5400,0x1,0,0,10,10,100,80,15.0,1,10\n"
        );
        std::fs::write(&path, rows).unwrap();

        let runs = runs(&path);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].matches, 2);
        assert_eq!(runs[0].goals_per_90, 3.0);
        assert_eq!(runs[0].shots_per_90, 20.0);
        // 15 a puerta de 40 tiros, no la media de 50 % y 33 %
        assert_eq!(runs[0].on_target, 15.0 / 40.0);
        assert_eq!(runs[0].pass_completion, 110.0 / 400.0);
        assert_eq!(runs[1].sha, "sha2");
    }

    #[test]
    fn an_empty_log_says_so_instead_of_failing() {
        let missing = std::env::temp_dir().join("gpf-no-such-measurements.csv");
        let _ = std::fs::remove_file(&missing);
        assert_eq!(compare_last_two(&missing), "sin mediciones registradas");
    }
}
