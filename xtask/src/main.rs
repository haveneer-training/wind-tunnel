//! Outillage du fil rouge.
//!
//! Le dépôt livré contient le code complet. Le stagiaire s'en fabrique un dossier de
//! travail, où les passages à écrire sont remplacés par des `todo!()` :
//!
//! ```shell
//! cargo xtask start                   # crée travail/
//! cd travail
//! ```
//!
//! Chaque bloc encadré par `// SOLUTION-BEGIN` / `// SOLUTION-END` y devient un
//! `todo!()` entouré de marqueurs, et le texte d'origine part dans
//! `xtask/reference.txt` — c'est de là que viennent les rattrapages.
//!
//! Depuis ce dossier de travail, il pilote ensuite son avancement :
//!
//! ```shell
//! cargo xtask status                  # où j'en suis
//! cargo xtask goto 3                  # passer à l'étape 3
//! cargo xtask next                    # passer à l'étape suivante
//! cargo xtask solve 2                 # remplir l'étape 2 pour moi (rattrapage)
//! cargo xtask reset 2                 # la rouvrir pour la refaire
//! ```
//!
//! Deux garde-fous, parce que ces commandes sont lancées par quelqu'un qui découvre le
//! projet : `goto` ne remplit que les trous **restés vides**, et `start` refuse
//! d'écraser un dossier de travail existant sans `--force`. `start` écrit dans
//! `travail/` par défaut ; `--dir <chemin>` en choisit un autre.
//!
//! Les exercices courts (`exercices/<m>`) suivent le même principe : le dépôt en
//! contiendrait les corrigés, annotés, dont `start` tirerait les énoncés (voir
//! [`exercises`]). La soufflerie n'en a pas ; le mécanisme reste, commun aux fils rouges.
//!
//! Pendant la mise au point des exercices, `cargo xtask update [--dir <chemin>]`, lancé
//! depuis le corrigé, reporte ses modifications dans un dossier de travail existant sans
//! perdre les trous déjà remplis, les exercices courts modifiés, ni l'étape courante.
//!
//! Organisation : [`cli`] analyse la ligne de commande et aiguille vers [`starter`]
//! (`start`), [`steps`] (`status`, `goto`, `next`, `solve`, `reset`), [`update`] et
//! [`verify`] ; tous s'appuient sur [`blocks`] (les blocs à compléter), [`exercises`]
//! (les énoncés des exercices courts) et [`files`] (le parcours du dépôt).

mod blocks;
mod cli;
mod exercises;
mod files;
mod starter;
mod steps;
mod update;
mod verify;

use crate::cli::{Command, parse, run, usage};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = match parse(&args) {
        Ok(Command::Help) => {
            print!("{}", usage());
            return;
        }
        Ok(command) => command,
        Err(e) => {
            eprintln!("erreur : {e}\n\n{}", usage());
            std::process::exit(2);
        }
    };

    if let Err(e) = run(command, &repository_root()) {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}

/// Racine du dépôt : le répertoire parent de celui du `Cargo.toml` de xtask.
fn repository_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}
