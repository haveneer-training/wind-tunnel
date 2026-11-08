//! Commandes du dossier de travail : avancement et progression des étapes.

use crate::blocks::{
    find_blocks, hole_body, is_empty, rewrite, REGION_BEGIN, REGION_END, SOLUTION_BEGIN,
};
use crate::files::{relative, source_files};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const REFERENCE: &str = "xtask/reference.txt";
pub const LAST_STEP: u8 = 12;
const STEP_MANIFEST: &str = "Cargo.toml";

pub fn load_reference(root: &Path) -> Result<BTreeMap<(String, usize), Vec<String>>, String> {
    let path = root.join(REFERENCE);
    let text = fs::read_to_string(&path).map_err(|error| {
        format!(
            "{} illisible ({error}).\nCette commande s'utilise dans le dépôt de travail, pas dans le corrigé.",
            path.display()
        )
    })?;
    let mut map = BTreeMap::new();
    let mut key: Option<(String, usize)> = None;
    let mut body = Vec::new();

    for line in text.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            let fields: Vec<&str> = header.split(" @@ ").collect();
            if fields.len() == 3 {
                key = Some((
                    fields[0].to_string(),
                    fields[2].parse().map_err(|_| "numéro d'ordre illisible")?,
                ));
                body.clear();
            }
            continue;
        }
        if line == "@@" {
            if let Some(key) = key.take() {
                map.insert(key, std::mem::take(&mut body));
            }
            continue;
        }
        body.push(line.to_string());
    }
    Ok(map)
}

pub fn fill(root: &Path, step: u8, force: bool) -> Result<(), String> {
    let reference = load_reference(root)?;
    let mut total = 0;
    for path in source_files(root) {
        let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let blocks = find_blocks(&text, REGION_BEGIN, REGION_END);
        let key = relative(root, &path);
        let (new_text, changed) = rewrite(&text, &blocks, |block| {
            if block.step != step || (!force && !is_empty(block)) {
                return None;
            }
            reference.get(&(key.clone(), block.ordinal)).cloned()
        });
        if changed > 0 {
            fs::write(&path, new_text).map_err(|error| error.to_string())?;
            println!("  {key} : {changed} bloc(s) rempli(s)");
            total += changed;
        }
    }
    if total == 0 {
        println!("  étape {step} : rien à remplir");
    }
    Ok(())
}

pub fn reset(root: &Path, step: u8) -> Result<(), String> {
    for path in source_files(root) {
        let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let blocks = find_blocks(&text, REGION_BEGIN, REGION_END);
        let (new_text, changed) = rewrite(&text, &blocks, |block| {
            (block.step == step && !is_empty(block)).then(|| hole_body(block))
        });
        if changed > 0 {
            fs::write(&path, new_text).map_err(|error| error.to_string())?;
            println!("  {} : {changed} bloc(s) rouvert(s)", relative(root, &path));
        }
    }
    Ok(())
}

pub fn goto(root: &Path, step: u8) -> Result<(), String> {
    println!("Passage à l'étape {step}.");
    for earlier in 0..step {
        fill(root, earlier, false)?;
    }
    set_default_feature(root, step)?;
    println!(
        "Les tests des étapes 0 à {step} sont actifs. Lancez `cargo test`, puis complétez les `todo!()` de l'étape {step}."
    );
    Ok(())
}

pub fn next(root: &Path) -> Result<(), String> {
    let step = current_step(root).ok_or_else(|| {
        format!("étape courante introuvable (ligne `default = [...]` de {STEP_MANIFEST})")
    })?;
    match next_step(step) {
        Some(next) => goto(root, next),
        None if step == LAST_STEP => Err(format!(
            "vous êtes déjà à la dernière étape ({LAST_STEP}) : il n'y a pas d'étape suivante"
        )),
        None => Err(
            "ce dépôt est le corrigé : il n'y a pas d'étape à ouvrir.\nEngendrez le dépôt de travail avec `cargo xtask start`."
                .to_string(),
        ),
    }
}

fn next_step(step: u8) -> Option<u8> {
    (step < LAST_STEP).then_some(step + 1)
}

pub fn set_default_feature(root: &Path, step: u8) -> Result<(), String> {
    rewrite_default(&root.join(STEP_MANIFEST), &format!("[\"step{step}\"]"))
}

pub fn rewrite_default(path: &Path, value: &str) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut replaced = false;
    let new_text: Vec<String> = text
        .lines()
        .map(|line| {
            if line.starts_with("default = [") && !replaced {
                replaced = true;
                format!("default = {value}")
            } else {
                line.to_string()
            }
        })
        .collect();
    if !replaced {
        return Err(format!(
            "ligne `default = [...]` introuvable dans {}",
            path.display()
        ));
    }
    fs::write(path, new_text.join("\n") + "\n").map_err(|error| error.to_string())
}

pub fn current_step(root: &Path) -> Option<u8> {
    let text = fs::read_to_string(root.join(STEP_MANIFEST)).ok()?;
    let line = text.lines().find(|line| line.starts_with("default = ["))?;
    let (_, rest) = line.split_once("step")?;
    rest.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

pub fn status(root: &Path) -> Result<(), String> {
    let mut done: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
    let mut corrected = false;
    for path in source_files(root) {
        let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        corrected |= text.contains(SOLUTION_BEGIN);
        for block in find_blocks(&text, REGION_BEGIN, REGION_END) {
            let entry = done.entry(block.step).or_insert((0, 0));
            entry.1 += 1;
            if !is_empty(&block) {
                entry.0 += 1;
            }
        }
    }
    if corrected {
        println!("Ce dépôt est le corrigé : il n'y a rien à compléter.");
        println!("Engendrez le dépôt de travail avec `cargo xtask start`.");
        return Ok(());
    }

    let step = current_step(root).unwrap_or(0);
    println!("Étape courante : {step}\n");
    for number in 0..=LAST_STEP {
        let (filled, total) = done.get(&number).copied().unwrap_or((0, 0));
        let mark = match (total, filled == total) {
            (0, _) => "  ",
            (_, true) => "OK",
            _ => "→ ",
        };
        let active = if number <= step { "tests actifs" } else { "" };
        println!("  {mark} étape {number} : {filled}/{total} bloc(s) complété(s)  {active}");
    }
    println!("\n`cargo xtask next` pour passer à la suivante, `cargo xtask goto <n>` pour changer d'étape.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_step_stops_at_the_last_one() {
        assert_eq!(next_step(0), Some(1));
        assert_eq!(next_step(LAST_STEP - 1), Some(LAST_STEP));
        assert_eq!(next_step(LAST_STEP), None);
        assert_eq!(next_step(LAST_STEP + 1), None);
    }

    #[test]
    fn the_manifest_ends_with_the_sentinel() {
        let manifest = include_str!("../../Cargo.toml");
        let highest = manifest
            .lines()
            .filter_map(|line| line.strip_prefix("step")?.split_once(" = ")?.0.parse().ok())
            .max();
        assert_eq!(highest, Some(LAST_STEP + 1));
    }
}
