//! Report des modifications du corrigé dans un dossier de travail (`cargo xtask update`).

use crate::blocks::{find_blocks, is_empty, rewrite, Block, REGION_BEGIN, REGION_END};
use crate::files::{relative, source_files};
use crate::starter::{ensure_reference, generate, STARTER_ENTRIES};
use crate::steps::{current_step, load_reference, set_default_feature, REFERENCE};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub fn update(root: &Path, out: &Path) -> Result<(), String> {
    ensure_reference(root)?;
    if !out.join(REFERENCE).is_file() {
        return Err(format!(
            "{} n'est pas un dossier de travail ({REFERENCE} introuvable).\nCréez-le avec `cargo xtask start`.",
            out.display()
        ));
    }
    let out = fs::canonicalize(out).map_err(|error| format!("{} : {error}", out.display()))?;
    let name = out
        .file_name()
        .ok_or_else(|| format!("{} : chemin sans nom de dossier", out.display()))?;
    let staging = out.with_file_name(format!(".{}-update", name.to_string_lossy()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| format!("{} : {error}", staging.display()))?;
    }
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let result = update_into(root, &out, &staging);
    let _ = fs::remove_dir_all(staging);
    result
}

fn update_into(root: &Path, out: &Path, staging: &Path) -> Result<(), String> {
    let holes = generate(root, staging)?;
    let old_reference = load_reference(out)?;
    let new_reference = load_reference(staging)?;
    let mut kept = 0;
    let mut lost = Vec::new();

    for path in source_files(staging) {
        let key = relative(staging, &path);
        let Ok(old_text) = fs::read_to_string(out.join(&key)) else {
            continue;
        };
        let new_text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let old_blocks = find_blocks(&old_text, REGION_BEGIN, REGION_END);
        let new_blocks = find_blocks(&new_text, REGION_BEGIN, REGION_END);
        let old_keys = block_keys(&old_text, &old_blocks);
        let new_keys = block_keys(&new_text, &new_blocks);
        let old_by_key: BTreeMap<(u8, String, usize), &Block> =
            old_keys.iter().cloned().zip(&old_blocks).collect();
        let mut matched = BTreeSet::new();

        let (merged, changed) = rewrite(&new_text, &new_blocks, |block| {
            let identity = &new_keys[block.ordinal];
            let old = old_by_key.get(identity)?;
            matched.insert(identity.clone());
            if is_empty(old) {
                return None;
            }
            if old_reference.get(&(key.clone(), old.ordinal)) == Some(&old.body) {
                return new_reference.get(&(key.clone(), block.ordinal)).cloned();
            }
            Some(old.body.clone())
        });
        for (identity, old) in &old_by_key {
            if !matched.contains(identity) && !is_empty(old) {
                lost.push(format!(
                    "{key} ligne {} (étape {}) : ce trou n'existe plus dans le corrigé",
                    old.start + 1,
                    old.step
                ));
            }
        }
        if changed > 0 {
            fs::write(&path, merged).map_err(|error| error.to_string())?;
            println!("  {key} : {changed} bloc(s) conservé(s)");
            kept += changed;
        }
    }
    for path in source_files(out) {
        let key = relative(out, &path);
        if staging.join(&key).exists() {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        if find_blocks(&text, REGION_BEGIN, REGION_END)
            .iter()
            .any(|block| !is_empty(block))
        {
            lost.push(format!("{key} : ce fichier n'existe plus dans le corrigé"));
        }
    }
    if !lost.is_empty() {
        return Err(format!(
            "mise à jour abandonnée, du travail serait perdu :\n  {}\nRouvrez ces trous (`cargo xtask reset <n>` dans le dossier de travail) ou sauvegardez votre travail, puis relancez.",
            lost.join("\n  ")
        ));
    }

    let step = current_step(out).unwrap_or(0);
    set_default_feature(staging, step)?;
    for entry in STARTER_ENTRIES.into_iter().chain([REFERENCE]) {
        let (from, to) = (staging.join(entry), out.join(entry));
        if to.is_dir() {
            fs::remove_dir_all(&to).map_err(|error| format!("{} : {error}", to.display()))?;
        } else if to.exists() {
            fs::remove_file(&to).map_err(|error| format!("{} : {error}", to.display()))?;
        }
        if from.exists() {
            fs::rename(&from, &to).map_err(|error| format!("{} : {error}", to.display()))?;
        }
    }
    let shown = out.strip_prefix(root).unwrap_or(out);
    println!(
        "Dossier de travail mis à jour : {} ({holes} trous, {kept} rempli(s) conservé(s), étape {step}).",
        shown.display()
    );
    Ok(())
}

fn block_keys(text: &str, blocks: &[Block]) -> Vec<(u8, String, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut seen: BTreeMap<(u8, String), usize> = BTreeMap::new();
    blocks
        .iter()
        .map(|block| {
            let name = lines[..block.start]
                .iter()
                .rev()
                .find_map(|line| {
                    let signature = line.trim();
                    let signature = signature
                        .strip_prefix("pub(crate) ")
                        .or_else(|| signature.strip_prefix("pub "))
                        .unwrap_or(signature)
                        .strip_prefix("fn ")?;
                    signature
                        .split(|character: char| {
                            character == '(' || character == '<' || character.is_whitespace()
                        })
                        .next()
                        .filter(|name| !name.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| format!("bloc-{}", block.ordinal));
            let identity = (block.step, name);
            let rank = seen.entry(identity.clone()).or_insert(0);
            let key = (identity.0, identity.1, *rank);
            *rank += 1;
            key
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::starter::make_starter;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempProject(PathBuf);

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp_project() -> TempProject {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join(format!(".xtask-update-test-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        TempProject(path)
    }

    #[test]
    fn inserting_an_exercise_does_not_change_existing_function_keys() {
        let old = "fn build_faces() {\n    // >>> ÉTAPE 2 — à compléter\n    todo!(\"étape 2\")\n    // <<< ÉTAPE 2\n}\n";
        let new = "fn edge_key() {\n    // >>> ÉTAPE 2 — à compléter\n    todo!(\"étape 2\")\n    // <<< ÉTAPE 2\n}\nfn build_faces() {\n    // >>> ÉTAPE 2 — à compléter\n    todo!(\"étape 2\")\n    // <<< ÉTAPE 2\n}\n";
        let old_blocks = find_blocks(old, REGION_BEGIN, REGION_END);
        let new_blocks = find_blocks(new, REGION_BEGIN, REGION_END);
        assert_eq!(
            block_keys(old, &old_blocks)[0],
            block_keys(new, &new_blocks)[1]
        );
    }

    #[test]
    fn update_keeps_a_custom_answer_when_a_new_hole_is_inserted_before_it() {
        let temp = temp_project();
        let root = &temp.0.join("reference");
        let work = &temp.0.join("work");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("mpi")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[features]\ndefault = [\"step13\"]\n",
        )
        .unwrap();
        fs::write(
            root.join("mpi/Cargo.toml"),
            "[features]\ndefault = [\"step13\"]\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "fn build_faces() {\n    // TODO-STEP:2 Construire les faces.\n    // SOLUTION-BEGIN\n    let reference_answer = true;\n    // SOLUTION-END\n}\n",
        )
        .unwrap();
        make_starter(root, work, false).unwrap();
        let work_source = work.join("src/lib.rs");
        let trainee_text = fs::read_to_string(&work_source).unwrap().replace(
            "todo!(\"étape 2 — voir le commentaire ci-dessus\")",
            "let custom_answer = true;",
        );
        fs::write(&work_source, trainee_text).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "fn edge_key() {\n    // TODO-STEP:2 Canonicaliser l'arête.\n    // SOLUTION-BEGIN\n    let edge_answer = true;\n    // SOLUTION-END\n}\nfn build_faces() {\n    // TODO-STEP:2 Construire les faces.\n    // SOLUTION-BEGIN\n    let reference_answer = true;\n    // SOLUTION-END\n}\n",
        )
        .unwrap();

        update(root, work).unwrap();

        let updated = fs::read_to_string(work_source).unwrap();
        assert!(updated.contains("let custom_answer = true;"));
        assert!(updated.contains("fn edge_key()"));
        assert!(!updated.contains("let edge_answer = true;"));
        assert!(updated.contains("todo!(\"étape 2 — voir le commentaire ci-dessus\")"));
        assert!(fs::read_to_string(work.join("Cargo.toml"))
            .unwrap()
            .contains("default = [\"step0\"]"));
    }
}
