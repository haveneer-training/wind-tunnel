//! Outillage du fil rouge.
//!
//! Le dépôt livré contient le code complet. Le stagiaire s'en fabrique un dossier de
//! travail, où les passages à écrire sont remplacés par des `todo!()` :
//!
//! ```shell
//! cargo xtask starter                 # crée travail/
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
//! cargo xtask solve 2                 # remplir l'étape 2 pour moi (rattrapage)
//! cargo xtask reset 2                 # la rouvrir pour la refaire
//! ```
//!
//! Deux garde-fous, parce que ces commandes sont lancées par quelqu'un qui découvre le
//! projet : `goto` ne remplit que les trous **restés vides**, et `starter` refuse
//! d'écraser un dossier de travail existant sans `--force`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Marqueur ouvrant dans le dépôt corrigé.
const SOLUTION_BEGIN: &str = "// SOLUTION-BEGIN";
/// Marqueur fermant dans le dépôt corrigé.
const SOLUTION_END: &str = "// SOLUTION-END";
/// Préfixe du marqueur ouvrant dans le dépôt de travail.
const REGION_BEGIN: &str = "// >>> ÉTAPE ";
/// Préfixe du marqueur fermant dans le dépôt de travail.
const REGION_END: &str = "// <<< ÉTAPE ";
/// Où sont rangés les blocs de référence, dans le dépôt de travail.
const REFERENCE: &str = "xtask/reference.txt";
/// Dernière étape couverte par le code actuel.
const LAST_STEP: u8 = 11;
/// Nom du dossier de travail engendré, à la racine du dépôt.
const WORKDIR: &str = "travail";

/// Un bloc à compléter, repéré dans un fichier source.
struct Block {
    step: u8,
    /// Indentation à réutiliser pour le `todo!()`.
    indent: String,
    /// Lignes du corps, marqueurs exclus.
    body: Vec<String>,
    /// Numéro d'ordre du bloc dans son fichier.
    ordinal: usize,
    /// Ligne du marqueur ouvrant.
    start: usize,
    /// Ligne du marqueur fermant.
    end: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let root = repository_root();

    let result = match command {
        "starter" => {
            let force = args.iter().any(|a| a == "--force");
            let out = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with("--"))
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(WORKDIR));
            make_starter(&root, &out, force)
        }
        "status" => status(&root),
        "goto" => parse_step(args.get(1)).and_then(|n| goto(&root, n)),
        "solve" => parse_step(args.get(1)).and_then(|n| fill(&root, n, true)),
        "reset" => parse_step(args.get(1)).and_then(|n| reset(&root, n)),
        _ => {
            print!("{}", usage());
            return;
        }
    };

    if let Err(e) = result {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}

fn usage() -> String {
    format!(
        "Outillage du fil rouge « soufflerie numérique »\n\
         \n\
           cargo xtask status        où en est le projet, étape par étape\n\
           cargo xtask goto <n>      passer à l'étape n (0 à {LAST_STEP})\n\
           cargo xtask solve <n>     remplir les trous de l'étape n\n\
           cargo xtask reset <n>     rouvrir les trous de l'étape n\n\
           cargo xtask starter       créer le dossier de travail (par défaut : {WORKDIR}/)\n\
         \n\
         Les quatre premières commandes s'utilisent depuis le dossier de travail,\n\
         `starter` depuis le dépôt du corrigé.\n"
    )
}

fn parse_step(arg: Option<&String>) -> Result<u8, String> {
    let raw = arg.ok_or_else(|| format!("il manque le numéro d'étape\n\n{}", usage()))?;
    let n: u8 = raw
        .parse()
        .map_err(|_| format!("« {raw} » n'est pas un numéro d'étape"))?;
    if n > LAST_STEP {
        return Err(format!("l'étape {n} n'existe pas encore (0 à {LAST_STEP})"));
    }
    Ok(n)
}

/// Racine du dépôt : le répertoire parent de celui du `Cargo.toml` de xtask.
fn repository_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(Path::to_path_buf).unwrap_or(manifest)
}

/// Répertoires de sources fouillés par le dispositif d'étapes.
///
/// `mpi/src` en fait partie depuis l'étape 11 : le pilote MPI est un crate à part, membre
/// du workspace mais pas du groupe par défaut (le reste se construit sans MPI) — ses
/// trous sont des trous comme les autres, et `goto`/`solve`/`reset`/`status` doivent les
/// voir.
const SOURCE_DIRS: [&str; 2] = ["src", "mpi/src"];

/// Liste les fichiers `.rs` de toutes les sources du dépôt.
fn source_files(root: &Path) -> Vec<PathBuf> {
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

/// Repère les blocs délimités par `begin` / `end` dans un fichier source.
///
/// L'étape est lue sur le commentaire `TODO-STEP:<n>` qui précède le bloc dans le
/// dépôt corrigé, ou sur le marqueur lui-même dans le dépôt de travail.
fn find_blocks(text: &str, begin: &str, end: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut ordinal = 0;
    let mut i = 0;

    while i < lines.len() {
        if !lines[i].trim_start().starts_with(begin) {
            i += 1;
            continue;
        }
        let start = i;
        let indent: String = lines[i].chars().take_while(|c| c.is_whitespace()).collect();

        let step = step_of(&lines, start, begin).unwrap_or(0);
        let Some(stop) = (start + 1..lines.len()).find(|&k| lines[k].trim_start().starts_with(end))
        else {
            break;
        };

        blocks.push(Block {
            step,
            indent,
            body: lines[start + 1..stop]
                .iter()
                .map(|l| l.to_string())
                .collect(),
            ordinal,
            start,
            end: stop,
        });
        ordinal += 1;
        i = stop + 1;
    }
    blocks
}

/// Numéro d'étape associé à un bloc.
fn step_of(lines: &[&str], start: usize, begin: &str) -> Option<u8> {
    // dépôt de travail : le numéro est sur le marqueur lui-même
    if begin == REGION_BEGIN {
        return lines[start]
            .trim_start()
            .strip_prefix(REGION_BEGIN)?
            .split_whitespace()
            .next()?
            .parse()
            .ok();
    }
    // dépôt corrigé : on remonte jusqu'au commentaire TODO-STEP le plus proche
    let floor = start.saturating_sub(8);
    (floor..start).rev().find_map(|k| {
        let (_, rest) = lines[k].split_once("TODO-STEP:")?;
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

/// Reconstruit un fichier en remplaçant le corps de certains blocs.
///
/// `replacement` reçoit un bloc et renvoie les lignes à mettre à sa place, ou `None`
/// pour le laisser tel quel.
fn rewrite(
    text: &str,
    blocks: &[Block],
    mut replacement: impl FnMut(&Block) -> Option<Vec<String>>,
) -> (String, usize) {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut changed = 0;
    let mut i = 0;

    while i < lines.len() {
        match blocks.iter().find(|b| b.start == i) {
            Some(block) => match replacement(block) {
                Some(body) => {
                    out.push(lines[block.start].to_string());
                    out.extend(body);
                    out.push(lines[block.end].to_string());
                    changed += 1;
                    i = block.end + 1;
                }
                None => {
                    out.push(lines[i].to_string());
                    i += 1;
                }
            },
            None => {
                out.push(lines[i].to_string());
                i += 1;
            }
        }
    }
    (out.join("\n") + "\n", changed)
}

/// La ligne `todo!()` qui tient lieu de travail à faire.
fn todo_line(indent: &str, step: u8) -> String {
    format!("{indent}todo!(\"étape {step} — voir le commentaire ci-dessus\")")
}

// ---------------------------------------------------------------- dépôt de travail

/// Charge les blocs de référence : `(chemin relatif, numéro d'ordre) → corps`.
fn load_reference(root: &Path) -> Result<BTreeMap<(String, usize), Vec<String>>, String> {
    let path = root.join(REFERENCE);
    let text = fs::read_to_string(&path).map_err(|e| {
        format!(
            "{} illisible ({e}).\nCette commande s'utilise dans le dépôt de travail, \
             pas dans le corrigé.",
            path.display()
        )
    })?;

    let mut map = BTreeMap::new();
    let mut key: Option<(String, usize)> = None;
    let mut body: Vec<String> = Vec::new();

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
            if let Some(k) = key.take() {
                map.insert(k, std::mem::take(&mut body));
            }
            continue;
        }
        body.push(line.to_string());
    }
    Ok(map)
}

/// Chemin d'un fichier, relatif à la racine, avec des séparateurs `/`.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Un bloc est « vide » tant qu'il contient encore son `todo!()`.
fn is_empty(block: &Block) -> bool {
    block.body.iter().any(|l| l.contains("todo!("))
}

/// Remplit les trous d'une étape ; `force` écrase même ce que le stagiaire a écrit.
fn fill(root: &Path, step: u8, force: bool) -> Result<(), String> {
    let reference = load_reference(root)?;
    let files = source_files(root);

    let mut total = 0;
    for path in files {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let blocks = find_blocks(&text, REGION_BEGIN, REGION_END);
        let key = relative(root, &path);

        let (new_text, changed) = rewrite(&text, &blocks, |b| {
            if b.step != step || (!force && !is_empty(b)) {
                return None;
            }
            reference.get(&(key.clone(), b.ordinal)).cloned()
        });
        if changed > 0 {
            fs::write(&path, new_text).map_err(|e| e.to_string())?;
            println!("  {key} : {changed} bloc(s) rempli(s)");
            total += changed;
        }
    }
    if total == 0 {
        println!("  étape {step} : rien à remplir");
    }
    Ok(())
}

/// Rouvre les trous d'une étape.
fn reset(root: &Path, step: u8) -> Result<(), String> {
    let files = source_files(root);

    for path in files {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let blocks = find_blocks(&text, REGION_BEGIN, REGION_END);
        let (new_text, changed) = rewrite(&text, &blocks, |b| {
            (b.step == step && !is_empty(b)).then(|| vec![todo_line(&b.indent, b.step)])
        });
        if changed > 0 {
            fs::write(&path, new_text).map_err(|e| e.to_string())?;
            println!("  {} : {changed} bloc(s) rouvert(s)", relative(root, &path));
        }
    }
    Ok(())
}

/// Passe le projet à l'étape demandée.
fn goto(root: &Path, step: u8) -> Result<(), String> {
    println!("Passage à l'étape {step}.");
    for earlier in 0..step {
        fill(root, earlier, false)?;
    }
    set_default_feature(root, step)?;
    println!(
        "Les tests des étapes 0 à {step} sont actifs. Lancez `cargo test`, \
         puis complétez les `todo!()` de l'étape {step}."
    );
    Ok(())
}

/// Réécrit la ligne `default = [...]` du manifeste de la racine.
fn set_default_feature(root: &Path, step: u8) -> Result<(), String> {
    rewrite_default(&root.join("Cargo.toml"), &format!("[\"step{step}\"]"))
}

/// Remplace la valeur de la ligne `default = [...]` d'un manifeste.
fn rewrite_default(path: &Path, value: &str) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
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
    fs::write(path, new_text.join("\n") + "\n").map_err(|e| e.to_string())
}

/// Étape actuellement déclarée dans le manifeste.
fn current_step(root: &Path) -> Option<u8> {
    let text = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let line = text.lines().find(|l| l.starts_with("default = ["))?;
    let (_, rest) = line.split_once("step")?;
    rest.chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

/// Affiche l'avancement, étape par étape.
fn status(root: &Path) -> Result<(), String> {
    let files = source_files(root);

    let mut done: BTreeMap<u8, (usize, usize)> = BTreeMap::new();
    let mut corrected = false;
    for path in &files {
        let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
        if text.contains(SOLUTION_BEGIN) {
            corrected = true;
        }
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
        println!("Engendrez le dépôt de travail avec `cargo xtask starter`.");
        return Ok(());
    }

    let step = current_step(root).unwrap_or(0);
    println!("Étape courante : {step}\n");
    for n in 0..=LAST_STEP {
        let (filled, total) = done.get(&n).copied().unwrap_or((0, 0));
        let mark = match (total, filled == total) {
            (0, _) => "  ",
            (_, true) => "OK",
            _ => "→ ",
        };
        let active = if n <= step { "tests actifs" } else { "" };
        println!("  {mark} étape {n} : {filled}/{total} bloc(s) complété(s)  {active}");
    }
    println!("\n`cargo xtask goto <n>` pour changer d'étape.");
    Ok(())
}

// ------------------------------------------------------------------ génération

/// Engendre le dépôt de travail à partir du corrigé.
fn make_starter(root: &Path, out: &Path, force: bool) -> Result<(), String> {
    // Le dépôt du corrigé est la source : sans ses marqueurs, il n'y a rien à trouer.
    // Cette commande lancée depuis un dossier de travail ne produirait qu'une copie
    // sans exercices, ce qui serait une fausse bonne surprise.
    let is_reference = source_files(root).iter().any(|p| {
        fs::read_to_string(p)
            .map(|t| t.contains(SOLUTION_BEGIN))
            .unwrap_or(false)
    });
    if !is_reference {
        return Err(format!(
            "ce dépôt ne contient aucun bloc de référence : c'est déjà un dossier de \
             travail.\nLancez `cargo xtask starter` depuis le dépôt du corrigé, ou \
             `cargo xtask status` pour voir où vous en êtes."
        ));
    }

    // Ne jamais détruire le travail de quelqu'un sans le lui demander.
    if out.exists() && fs::read_dir(out).map(|d| d.count() > 0).unwrap_or(false) {
        if !force {
            return Err(format!(
                "{} existe déjà et n'est pas vide.\nSi c'est votre dossier de travail, \
                 il contient ce que vous avez écrit : ne le régénérez pas.\nPour le \
                 remplacer malgré tout — et perdre son contenu — ajoutez --force.",
                out.display()
            ));
        }
        fs::remove_dir_all(out).map_err(|e| format!("{} : {e}", out.display()))?;
    }
    fs::create_dir_all(out).map_err(|e| e.to_string())?;

    let mut reference = String::new();
    let mut holes = 0;

    for entry in [
        ".gitignore",
        "Cargo.toml",
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
        ".cargo",
    ] {
        copy_tree(&root.join(entry), &out.join(entry))?;
    }

    // Trouer les sources et mettre les corps de côté.
    for path in source_files(out) {
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
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
        let mut new_lines: Vec<String> = Vec::new();
        let mut i = 0;
        while i < lines.len() {
            match blocks.iter().find(|b| b.start == i) {
                Some(block) => {
                    new_lines.push(format!(
                        "{}{REGION_BEGIN}{} — à compléter",
                        block.indent, block.step
                    ));
                    new_lines.push(todo_line(&block.indent, block.step));
                    new_lines.push(format!("{}{REGION_END}{}", block.indent, block.step));
                    holes += 1;
                    i = block.end + 1;
                }
                None => {
                    new_lines.push(humanize_todo(lines[i]));
                    i += 1;
                }
            }
        }
        fs::write(&path, new_lines.join("\n") + "\n").map_err(|e| e.to_string())?;
    }

    fs::create_dir_all(out.join("xtask")).map_err(|e| e.to_string())?;
    fs::write(out.join(REFERENCE), reference).map_err(|e| e.to_string())?;

    // Le dépôt de travail démarre à l'étape 0.
    set_default_feature(out, 0)?;
    // La sentinelle `step12` ne vaut que pour le corrigé : c'est elle qui y désactive
    // les `allow` conditionnels posés sur les avertissements collatéraux des trous.
    // Un dossier de travail ne doit jamais l'activer, sinon ces avertissements
    // resteraient éteints alors même que le trou est encore ouvert.
    rewrite_default(&out.join("mpi/Cargo.toml"), "[]")?;
    if let Ok(readme) = fs::read_to_string(root.join("docs/README-travail.md")) {
        fs::write(out.join("README.md"), readme).map_err(|e| e.to_string())?;
    }

    let shown = out.strip_prefix(root).unwrap_or(out);
    println!(
        "Dossier de travail créé : {} ({holes} trous à combler).\n",
        shown.display()
    );
    println!("  cd {}", shown.display());
    println!("  cargo test           # quatre tests rouges : l'étape 0 vous attend");
    println!("  cargo xtask status   # à tout moment, pour savoir où vous en êtes\n");
    println!("L'énoncé de la première étape est dans docs/etapes/etape-00.md.");
    Ok(())
}

/// Transforme le marqueur machine du corrigé en consigne lisible.
///
/// `// TODO-STEP:3 Écrire l'en-tête` devient `// À FAIRE (étape 3) Écrire l'en-tête`.
fn humanize_todo(line: &str) -> String {
    let Some((before, after)) = line.split_once("TODO-STEP:") else {
        return line.to_string();
    };
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return line.to_string();
    }
    let rest = &after[digits.len()..];
    format!("{before}À FAIRE (étape {digits}){rest}")
}

/// Copie un fichier ou une arborescence.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    if from.is_dir() {
        fs::create_dir_all(to).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
            let name = entry.file_name();
            if name == "target" || name == "reference.txt" || name == "README-travail.md" {
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
