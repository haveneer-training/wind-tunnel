//! `cargo xtrain` : l'outillage du fil rouge « soufflerie numérique ».
//!
//! L'outillage est la crate commune `xtask-training`
//! (<https://github.com/haveneer-training/xtask-training>), dont la documentation décrit
//! les commandes. Ce fichier ne fait que décrire ce dépôt.

use xtask_training::{Project, StepCheck};

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
    // `cargo xtrain check-steps`. `--lib --tests` : les doctests ne suivent pas les
    // features `stepN`. `run` : le solveur, sur un petit maillage, ne doit jamais buter sur
    // le `todo!()` d'une étape déjà résolue (le défaut corrigé aux étapes 9 et 10).
    // `clippy` et `fmt` par défaut : le paquet racine, toutes ses cibles ; tout le
    // workspace (mpi/ et design/ compris).
    step_check: StepCheck {
        build_args: &[],
        test_args: &["--lib", "--tests"],
        run: Some(&[
            "--release",
            "--",
            "domains/tunnel.dom",
            "--refine",
            "2",
            "--bands",
            "3",
            "--max-steps",
            "3",
        ]),
        extra: Some("scripts/check-steps-extra.sh"),
        ..StepCheck::DEFAULT
    },
};

fn main() {
    xtask_training::main(&PROJECT, env!("CARGO_MANIFEST_DIR"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_project_matches_the_repository() {
        // La feature `stepN` la plus haute du manifeste doit être la sentinelle,
        // `last_step + 1` ; les chemins nommés doivent exister.
        xtask_training::check(&super::PROJECT, env!("CARGO_MANIFEST_DIR")).unwrap();
    }
}
