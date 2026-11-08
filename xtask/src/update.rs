//! Report des modifications du corrigé dans un dossier de travail (`cargo xtask update`).

use crate::blocks::{Block, REGION_BEGIN, REGION_END, find_blocks, is_empty, rewrite};
use crate::exercises::{DELIVERED, carry_over};
use crate::files::{relative, source_files};
use crate::starter::{STARTER_ENTRIES, ensure_reference, generate};
use crate::steps::{REFERENCE, current_step, load_reference, set_default_feature};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Reporte les dernières modifications du corrigé dans un dossier de travail existant.
///
/// Sert pendant la mise au point des exercices : le dossier de travail est engendré à
/// nouveau, à côté, puis ce que le stagiaire a écrit y est replacé avant qu'il ne remplace
/// l'ancien. Sont conservés : les trous remplis (ceux remplis par `solve` reprennent le
/// nouveau corrigé), les exercices courts modifiés (voir [`carry_over`]), l'étape
/// courante, et tout ce qui n'est pas tiré du corrigé (`target/`, `data/`, un éventuel
/// `.git/`). Tout le reste — documentation, tests, code hors des trous — est remplacé.
pub fn update(root: &Path, out: &Path) -> Result<(), String> {
    ensure_reference(root)?;
    if !out.join(REFERENCE).is_file() {
        return Err(format!(
            "{} n'est pas un dossier de travail ({REFERENCE} introuvable).\nCréez-le avec \
             `cargo xtask start`.",
            out.display()
        ));
    }
    let out = fs::canonicalize(out).map_err(|e| format!("{} : {e}", out.display()))?;
    let name = out
        .file_name()
        .ok_or_else(|| format!("{} : chemin sans nom de dossier", out.display()))?;
    // À côté du dossier de travail, pour que les renommages restent sur le même disque.
    let staging = out.with_file_name(format!(".{}-update", name.to_string_lossy()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| format!("{} : {e}", staging.display()))?;
    }
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let result = update_into(root, &out, &staging);
    let _ = fs::remove_dir_all(&staging);
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
        let new_text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let old_blocks = find_blocks(&old_text, REGION_BEGIN, REGION_END);
        let new_blocks = find_blocks(&new_text, REGION_BEGIN, REGION_END);

        // Les blocs se correspondent par leur rang au sein de leur étape : un trou ajouté
        // ou retiré à une étape ne décale pas ceux des autres.
        let old_ranks = step_ranks(&old_blocks);
        let new_ranks = step_ranks(&new_blocks);
        let old_by_rank: BTreeMap<(u8, usize), &Block> =
            old_ranks.iter().copied().zip(&old_blocks).collect();
        let mut matched = Vec::new();

        let (merged, changed) = rewrite(&new_text, &new_blocks, |b| {
            let rank = new_ranks[b.ordinal];
            let old = old_by_rank.get(&rank)?;
            matched.push(rank);
            if is_empty(old) {
                return None;
            }
            // Bloc rempli par `solve` : il suit le corrigé, nouvelle version comprise.
            if old_reference.get(&(key.clone(), old.ordinal)) == Some(&old.body) {
                return new_reference.get(&(key.clone(), b.ordinal)).cloned();
            }
            Some(old.body.clone())
        });
        for (rank, old) in &old_by_rank {
            if !matched.contains(rank) && !is_empty(old) {
                lost.push(format!(
                    "{key} ligne {} (étape {}) : ce trou n'existe plus dans le corrigé",
                    old.start + 1,
                    old.step
                ));
            }
        }
        if changed > 0 {
            fs::write(&path, merged).map_err(|e| e.to_string())?;
            println!("  {key} : {changed} bloc(s) conservé(s)");
            kept += changed;
        }
    }
    for path in source_files(out) {
        let key = relative(out, &path);
        if staging.join(&key).exists() {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if find_blocks(&text, REGION_BEGIN, REGION_END)
            .iter()
            .any(|b| !is_empty(b))
        {
            lost.push(format!("{key} : ce fichier n'existe plus dans le corrigé"));
        }
    }
    let exercises = carry_over(out, staging)?;
    lost.extend(exercises.lost);

    // Un remplissage perdu en silence serait la pire issue : on s'arrête avant d'avoir
    // touché au dossier de travail.
    if !lost.is_empty() {
        return Err(format!(
            "mise à jour abandonnée, du travail serait perdu :\n  {}\nRecopiez-le \
             ailleurs, rouvrez ces trous (`cargo xtask reset <n>` dans le dossier de \
             travail) ou supprimez ces exercices, puis relancez.",
            lost.join("\n  ")
        ));
    }

    let step = current_step(out).unwrap_or(0);
    set_default_feature(staging, step)?;

    for entry in STARTER_ENTRIES.into_iter().chain([REFERENCE, DELIVERED]) {
        let (from, to) = (staging.join(entry), out.join(entry));
        if to.is_dir() {
            fs::remove_dir_all(&to).map_err(|e| format!("{} : {e}", to.display()))?;
        } else if to.exists() {
            fs::remove_file(&to).map_err(|e| format!("{} : {e}", to.display()))?;
        }
        if from.exists() {
            fs::rename(&from, &to).map_err(|e| format!("{} : {e}", to.display()))?;
        }
    }

    for line in &exercises.kept {
        println!("  {line}");
    }
    let shown = out.strip_prefix(root).unwrap_or(out);
    println!(
        "Dossier de travail mis à jour : {} ({holes} trous, {kept} rempli(s) conservé(s), \
         étape {step}).",
        shown.display()
    );
    Ok(())
}

/// Pour chaque bloc, `(étape, rang parmi les blocs de cette étape)`, dans l'ordre des blocs.
fn step_ranks(blocks: &[Block]) -> Vec<(u8, usize)> {
    let mut seen: BTreeMap<u8, usize> = BTreeMap::new();
    blocks
        .iter()
        .map(|b| {
            let rank = seen.entry(b.step).or_insert(0);
            *rank += 1;
            (b.step, *rank - 1)
        })
        .collect()
}
