//! Génération du dossier de travail à partir du corrigé (`cargo xtask start`).

use crate::blocks::{
    find_blocks, hole_body, BLANK_NOTE, FILL_NOTE, REGION_BEGIN, REGION_END, SOLUTION_BEGIN,
    SOLUTION_END,
};
use crate::files::{copy_tree, markdown_files, relative, source_files};
use crate::steps::{rewrite_default, set_default_feature, REFERENCE};
use std::fs;
use std::path::Path;

pub const WORKDIR: &str = "travail";
const CORRIGE_BEGIN: &str = "<!-- CORRIGE-ONLY-BEGIN -->";
const CORRIGE_END: &str = "<!-- CORRIGE-ONLY-END -->";

pub const STARTER_ENTRIES: [&str; 19] = [
    ".gitignore",
    "Cargo.toml",
    "Cargo.lock",
    "ETAPES.md",
    "README.md",
    "docs",
    "domains",
    "examples",
    "src",
    "tests",
    "xtask/Cargo.toml",
    "xtask/src",
    "mpi/Cargo.toml",
    "mpi/src",
    "design/Cargo.toml",
    "design/src",
    "design/tests",
    ".cargo",
    "scripts",
];

pub fn ensure_reference(root: &Path) -> Result<(), String> {
    let is_reference = source_files(root).iter().any(|path| {
        fs::read_to_string(path)
            .map(|text| text.contains(SOLUTION_BEGIN))
            .unwrap_or(false)
    });
    if is_reference {
        Ok(())
    } else {
        Err(
            "ce dépôt ne contient aucun bloc de référence : c'est déjà un dossier de travail.\nLancez cette commande depuis le dépôt du corrigé, ou `cargo xtask status` pour voir où vous en êtes."
                .to_string(),
        )
    }
}

pub fn make_starter(root: &Path, out: &Path, force: bool) -> Result<(), String> {
    ensure_reference(root)?;
    if out.exists()
        && fs::read_dir(out)
            .map(|entries| entries.count() > 0)
            .unwrap_or(false)
    {
        if !force {
            return Err(format!(
                "{} existe déjà et n'est pas vide.\nPour le remplacer et perdre son contenu, ajoutez --force. Pour reporter les dernières modifications du corrigé, utilisez `cargo xtask update`.",
                out.display()
            ));
        }
        fs::remove_dir_all(out).map_err(|error| format!("{} : {error}", out.display()))?;
    }
    fs::create_dir_all(out).map_err(|error| error.to_string())?;
    let holes = generate(root, out)?;
    let shown = out.strip_prefix(root).unwrap_or(out);
    println!(
        "Dossier de travail créé : {} ({holes} trous à combler).\n",
        shown.display()
    );
    println!("  cd {}", shown.display());
    println!("  cargo test           # quatre tests rouges : l'étape 0 vous attend");
    println!("  cargo xtask status   # à tout moment, pour savoir où vous en êtes\n");
    println!("Le déroulement d'une étape est expliqué dans ETAPES.md ; l'énoncé de la première étape est dans docs/etapes/etape-00.md.");
    Ok(())
}

pub fn generate(root: &Path, out: &Path) -> Result<usize, String> {
    let mut reference = String::new();
    let mut holes = 0;
    for entry in STARTER_ENTRIES {
        copy_tree(&root.join(entry), &out.join(entry))?;
    }

    let mut docs = Vec::new();
    markdown_files(out, &mut docs);
    for path in docs {
        let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        if text.contains(CORRIGE_BEGIN) {
            fs::write(&path, strip_corrige_blocks(&text, &path)?)
                .map_err(|error| error.to_string())?;
        }
    }

    for path in source_files(out) {
        let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let blocks = find_blocks(&text, SOLUTION_BEGIN, SOLUTION_END);
        if blocks.is_empty() {
            continue;
        }
        let key = relative(out, &path);
        for block in &blocks {
            reference.push_str(&format!(
                "@@ {key} @@ {} @@ {}\n",
                block.step, block.ordinal
            ));
            for line in &block.body {
                reference.push_str(line);
                reference.push('\n');
            }
            reference.push_str("@@\n");
        }

        let lines: Vec<&str> = text.lines().collect();
        let mut new_lines = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            match blocks.iter().find(|block| block.start == index) {
                Some(block) => {
                    let note = if block.blank { BLANK_NOTE } else { FILL_NOTE };
                    new_lines.push(format!(
                        "{}{REGION_BEGIN}{} — {note}",
                        block.indent, block.step
                    ));
                    new_lines.extend(hole_body(block));
                    new_lines.push(format!("{}{REGION_END}{}", block.indent, block.step));
                    holes += 1;
                    index = block.end + 1;
                }
                None => {
                    new_lines.push(humanize_todo(lines[index]));
                    index += 1;
                }
            }
        }
        fs::write(&path, new_lines.join("\n") + "\n").map_err(|error| error.to_string())?;
    }

    fs::create_dir_all(out.join("xtask")).map_err(|error| error.to_string())?;
    fs::write(out.join(REFERENCE), reference).map_err(|error| error.to_string())?;
    set_default_feature(out, 0)?;
    rewrite_default(&out.join("mpi/Cargo.toml"), "[]")?;
    Ok(holes)
}

fn strip_corrige_blocks(text: &str, path: &Path) -> Result<String, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut kept = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim() == CORRIGE_BEGIN {
            let start = index;
            let Some(end) =
                (start + 1..lines.len()).find(|&line| lines[line].trim() == CORRIGE_END)
            else {
                return Err(format!(
                    "{} : {CORRIGE_BEGIN} sans {CORRIGE_END} (ligne {})",
                    path.display(),
                    start + 1
                ));
            };
            index = end + 1;
        } else {
            kept.push(lines[index]);
            index += 1;
        }
    }
    let mut merged: Vec<&str> = Vec::new();
    for line in kept {
        if line.is_empty() && merged.last() == Some(&"") {
            continue;
        }
        merged.push(line);
    }
    Ok(merged.join("\n") + "\n")
}

fn humanize_todo(line: &str) -> String {
    let Some((before, after)) = line.split_once("TODO-STEP:") else {
        return line.to_string();
    };
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return line.to_string();
    }
    format!("{before}À FAIRE (étape {digits}){}", &after[digits.len()..])
}
