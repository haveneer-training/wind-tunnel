//! Vérification des exercices courts (`cargo xtask verify`).

use crate::exercises::{SHORT_EXERCISES, exercise_files, statement};
use crate::starter::ensure_reference;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// Ligne d'en-tête qui annonce l'erreur attendue d'un énoncé qui ne doit pas compiler.
const EXPECTED_ERROR: &str = "Erreur attendue : ";

/// Vérifie les exercices courts, dans le dépôt du corrigé :
///
/// - chaque corrigé (`exercices/<m>/examples/*.rs`) compile, affiche « OK », et ses tests
///   éventuels passent ;
/// - chaque énoncé qu'en tire `cargo xtask start` est bien formaté, et soit échoue avec
///   l'erreur annoncée en tête (« Erreur attendue : … »), soit compile (des `todo!()`).
///
/// Les énoncés sont compilés à part, dans un dossier jetable : par `rustc` seul pour un
/// fichier autonome, dans une crate jetable pour un fichier qui importe une crate du dépôt
/// (`use wind_tunnel…`).
pub fn verify(root: &Path) -> Result<(), String> {
    ensure_reference(root)?;
    let scratch = std::env::temp_dir().join(format!("wind-tunnel-verify-{}", std::process::id()));
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let mut failures = Vec::new();
    for module in SHORT_EXERCISES {
        verify_solutions(root, module, &mut failures)?;
        verify_statements(root, module, &scratch, &mut failures)?;
    }
    let _ = fs::remove_dir_all(&scratch);
    let _ = fs::remove_dir_all(root.join("target").join("verify"));
    if failures.is_empty() {
        println!("Exercices courts : tout est conforme.");
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

/// Les corrigés sont les exemples de la crate `wind-tunnel-<module>`.
fn verify_solutions(root: &Path, module: &str, failures: &mut Vec<String>) -> Result<(), String> {
    let package = format!("wind-tunnel-{module}");
    let quiet = ["--quiet", "--color=never", "--examples", "-p", &package];
    let build = run(cargo(root).arg("build").args(quiet))?;
    if !build.status.success() {
        failures.push(format!(
            "corrigés {module} : ne compilent pas\n{}",
            String::from_utf8_lossy(&build.stderr)
        ));
        return Ok(());
    }
    for path in exercise_files(root, module) {
        let name = format!("{module}/{}", file_stem(&path));
        let binary = root
            .join("target")
            .join("debug")
            .join("examples")
            .join(file_stem(&path));
        let output = run(&mut Command::new(&binary))?;
        if output.status.success() && String::from_utf8_lossy(&output.stdout).contains("OK") {
            println!("  corrigé {name} : OK");
        } else {
            failures.push(format!(
                "corrigé {name} : échec à l'exécution\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    // Les exercices qui en ont (`#[cfg(test)]`) se vérifient aussi partie par partie.
    let tests = run(cargo(root).arg("test").args(quiet))?;
    if !tests.status.success() {
        failures.push(format!(
            "corrigés {module} : tests en échec\n{}{}",
            String::from_utf8_lossy(&tests.stdout),
            String::from_utf8_lossy(&tests.stderr)
        ));
    }
    Ok(())
}

/// Les énoncés tirés des corrigés, tels que `cargo xtask start` les livre.
fn verify_statements(
    root: &Path,
    module: &str,
    scratch: &Path,
    failures: &mut Vec<String>,
) -> Result<(), String> {
    let dir = scratch.join(module);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for path in exercise_files(root, module) {
        let stem = file_stem(&path);
        let name = format!("{module}/{stem}");
        let solution = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if !solution.contains("// SOLUTION-BEGIN") {
            failures.push(format!(
                "{name} : aucun bloc SOLUTION-BEGIN, l'énoncé serait le corrigé"
            ));
            continue;
        }
        let text = match statement(&solution) {
            Ok(text) => text,
            Err(e) => {
                failures.push(format!("{name} : {e}"));
                continue;
            }
        };
        let source = dir.join(format!("{stem}.rs"));
        fs::write(&source, &text).map_err(|e| e.to_string())?;

        let format = run(Command::new("rustfmt")
            .args(["--edition", "2024", "--check", "--color=never"])
            .arg(&source))?;
        if !format.status.success() {
            failures.push(format!(
                "énoncé {name} : mal formaté (marqueurs mal placés ?)\n{}",
                String::from_utf8_lossy(&format.stdout)
            ));
            continue;
        }

        let expected = text.lines().find_map(|l| {
            l.split_once(EXPECTED_ERROR)
                .map(|(_, e)| e.trim().to_string())
        });
        let output = if uses_workspace_crates(&text) {
            build_in_crate(root, module, &stem, &source)?
        } else {
            run(
                Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
                    .args(["--edition", "2024", "--error-format=short", "-o"])
                    .arg(dir.join(&stem))
                    .arg(&source),
            )?
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        match (expected, output.status.success()) {
            (Some(expected), true) => failures.push(format!(
                "énoncé {name} : compile, alors qu'il devrait échouer ({expected})"
            )),
            (Some(expected), false) if !stderr.contains(&expected) => failures.push(format!(
                "énoncé {name} : échoue, mais sans « {expected} »\n{stderr}"
            )),
            (Some(expected), false) => {
                println!("  énoncé {name} : échoue comme prévu ({expected})")
            }
            (None, true) => println!("  énoncé {name} : compile"),
            (None, false) => failures.push(format!("énoncé {name} : ne compile pas\n{stderr}")),
        }
    }
    Ok(())
}

/// Compile un énoncé qui importe une crate du dépôt, avec ses tests, dans une crate
/// jetable sous `target/verify/` qui a les mêmes dépendances que les exercices. Cargo
/// compile celles-ci au besoin, dans le `target/` du dépôt.
fn build_in_crate(root: &Path, module: &str, stem: &str, source: &Path) -> Result<Output, String> {
    let package = format!("verify-{module}-{stem}").replace('_', "-");
    let dir = root.join("target").join("verify").join(&package);
    fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
    fs::write(dir.join("Cargo.toml"), manifest(root, &package)).map_err(|e| e.to_string())?;
    fs::copy(source, dir.join("src").join("main.rs")).map_err(|e| e.to_string())?;
    run(cargo(root)
        .args([
            "build",
            "--all-targets",
            "--quiet",
            "--color=never",
            "--manifest-path",
        ])
        .arg(dir.join("Cargo.toml")))
}

/// Un exercice qui s'appuie sur le fil rouge l'importe.
fn uses_workspace_crates(text: &str) -> bool {
    text.lines().any(|l| l.starts_with("use wind_tunnel"))
}

/// `cargo`, lancé à la racine du dépôt (sa toolchain), avec le `target/` du dépôt.
fn cargo(root: &Path) -> Command {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"));
    command
}

fn run(command: &mut Command) -> Result<Output, String> {
    command
        .output()
        .map_err(|e| format!("{:?} : {e}", command.get_program()))
}

/// Le manifeste de la crate jetable d'un énoncé : les dépendances des exercices.
/// `[workspace]` en fait un workspace à part : elle est sous `target/`, donc sous la
/// racine du dépôt.
fn manifest(root: &Path, package: &str) -> String {
    let path = |dir: &str| root.join(dir).display().to_string().replace('\\', "/");
    format!(
        "[package]\n\
         name = \"{package}\"\n\
         version = \"0.0.0\"\n\
         edition = \"2024\"\n\
         publish = false\n\
         \n\
         [dependencies]\n\
         wind-tunnel = {{ path = '{}' }}\n\
         \n\
         [workspace]\n",
        path("."),
    )
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
