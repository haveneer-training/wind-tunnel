//! Parcours et copie des fichiers du dépôt.

use std::fs;
use std::path::{Path, PathBuf};

/// Répertoires de sources fouillés par le dispositif d'étapes.
///
/// La bibliothèque, le pilote MPI (étape 11) et le bonus de conception (`design/`). Les
/// répertoires `tests/` n'y sont pas : leurs fichiers sont copiés tels quels, ce qui
/// garantit qu'aucun test ne se fait trouer par mégarde.
const SOURCE_DIRS: [&str; 3] = ["src", "mpi/src", "design/src"];

/// Liste les fichiers `.rs` de toutes les sources du dépôt.
pub fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in SOURCE_DIRS {
        rust_files(&root.join(dir), &mut files);
    }
    files
}

/// Liste récursivement les fichiers `.rs` d'un répertoire.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

pub fn rust_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    rust_files(dir, &mut files);
    files
}

/// Liste récursivement les fichiers `.md` d'un répertoire.
///
/// Sert à retirer les blocs `CORRIGE-ONLY` de la documentation copiée dans le dossier de
/// travail — pas seulement du README, de n'importe quel fichier Markdown du dépôt.
pub fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            markdown_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// Chemin d'un fichier, relatif à la racine, avec des séparateurs `/`.
pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Copie un fichier ou une arborescence.
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_dir() {
        fs::create_dir_all(to).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
            let name = entry.file_name();
            if name == "target" || name == "reference.txt" {
                continue;
            }
            copy_tree(&entry.path(), &to.join(name))?;
        }
    } else if from.exists() {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(from, to).map_err(|e| format!("{} : {e}", from.display()))?;
    }
    Ok(())
}
