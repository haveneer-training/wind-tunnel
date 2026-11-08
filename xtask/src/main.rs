//! Outillage du fil rouge. Le dépôt livré contient le code complet ; `cargo xtask start`
//! en génère une copie de travail où les blocs à compléter deviennent des `todo!()`.
//!
//! Depuis le dépôt du corrigé :
//!
//! ```shell
//! cargo xtask start [--force] [--dir <chemin>]
//! ```
//!
//! Depuis ce dossier de travail, les commandes `status`, `next`, `goto`, `solve` et
//! `reset` pilotent la progression. Depuis le dépôt du corrigé, `cargo xtask update`
//! synchronise les fichiers sans remplacer les réponses déjà écrites.
//!
//! L’implémentation est répartie entre `cli` (arguments et dispatch), `blocks`
//! (marqueurs d’exercice), `files` (parcours et copie), `starter` (génération),
//! `steps` (progression) et `update` (synchronisation).

mod blocks;
mod cli;
mod files;
mod starter;
mod steps;
mod update;

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match cli::parse(&args) {
        Ok(cli::Command::Help) => {
            print!("{}", cli::usage());
            return;
        }
        Ok(command) => command,
        Err(error) => {
            eprintln!("erreur : {error}\n\n{}", cli::usage());
            std::process::exit(2);
        }
    };

    if let Err(error) = cli::run(command, &repository_root()) {
        eprintln!("erreur : {error}");
        std::process::exit(1);
    }
}

/// Racine du dépôt : le répertoire parent de celui du `Cargo.toml` de xtask.
fn repository_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}
