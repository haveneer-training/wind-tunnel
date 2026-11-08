//! Parcours et copie des fichiers du dépôt.

use std::fs;
use std::path::{Path, PathBuf};

const SOURCE_DIRS: [&str; 3] = ["src", "mpi/src", "design/src"];

pub fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in SOURCE_DIRS {
        rust_files(&root.join(dir), &mut files);
    }
    files
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

pub fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            markdown_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
}

pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_dir() {
        fs::create_dir_all(to).map_err(|error| error.to_string())?;
        for entry in fs::read_dir(from)
            .map_err(|error| error.to_string())?
            .flatten()
        {
            let name = entry.file_name();
            if name == "target" || name == "reference.txt" {
                continue;
            }
            copy_tree(&entry.path(), &to.join(name))?;
        }
    } else if from.exists() {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(from, to).map_err(|error| format!("{} : {error}", from.display()))?;
    }
    Ok(())
}
