//! `cargo xtrain` : l'outillage du fil rouge « soufflerie numérique ».
//!
//! L'outillage est la crate commune `xtask-training`
//! (<https://github.com/haveneer-training/xtask-training>), dont la documentation décrit
//! les commandes. Ce fichier ne fait que décrire ce dépôt.

use xtask_training::Project;

const PROJECT: Project = Project {
    name: "soufflerie numérique",
    // `Cargo.toml` déclare une feature de plus, la sentinelle `step13`.
    last_step: 12,
    step_manifest: "Cargo.toml",
    // Le pilote MPI (`mpi/`, étape 11) suit les features de la bibliothèque : le dossier
    // de travail vide sa ligne `default`, `goto` n'y touche pas.
    sentinel_manifests: &["mpi/Cargo.toml"],
    // La bibliothèque, le pilote MPI (étape 11) et le bonus de conception (`design/`).
    source_dirs: &["src", "mpi/src", "design/src"],
    // `travail/` lui-même n'en fait jamais partie, ni `solutions/`, ni `target/`, ni les
    // sorties d'un calcul.
    starter_entries: &[
        ".gitignore",
        "rust-toolchain.toml",
        "Cargo.toml",
        "Cargo.lock",
        "ETAPES.md",
        "README.md",
        "docs",
        "domains",
        "examples",
        "src",
        "tests",
        "xtrain/Cargo.toml",
        "xtrain/src",
        "mpi/Cargo.toml",
        "mpi/src",
        "design/Cargo.toml",
        "design/src",
        "design/tests",
        ".cargo",
        "scripts",
    ],
    // La soufflerie n'a ni corrigés hors fil rouge, ni outillage réservé au formateur, ni
    // exercices courts : le fil rouge est son seul exercice.
    corrige_trees: &[],
    corrige_only_paths: &[],
    short_exercises: &[],
    exercise_package_prefix: "wind-tunnel",
    exercise_dependencies: &[("wind-tunnel", ".")],
    exercise_manifest_extra: "",
};

fn main() {
    xtask_training::main(&PROJECT, env!("CARGO_MANIFEST_DIR"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_project_matches_the_repository() {
        // `scripts/check-steps.sh` déduit la dernière étape du manifeste : sa feature
        // `stepN` la plus haute doit être la sentinelle, `last_step + 1`.
        xtask_training::check(&super::PROJECT, env!("CARGO_MANIFEST_DIR")).unwrap();
    }
}
