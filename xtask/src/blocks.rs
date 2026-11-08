//! Blocs à compléter : repérage des marqueurs et réécriture des fichiers.

pub const SOLUTION_BEGIN: &str = "// SOLUTION-BEGIN";
pub const SOLUTION_END: &str = "// SOLUTION-END";
pub const REGION_BEGIN: &str = "// >>> ÉTAPE ";
pub const REGION_END: &str = "// <<< ÉTAPE ";
pub const FILL_NOTE: &str = "à compléter";
pub const BLANK_NOTE: &str = "à écrire de zéro";

pub struct Block {
    pub step: u8,
    pub blank: bool,
    pub indent: String,
    pub body: Vec<String>,
    pub ordinal: usize,
    pub start: usize,
    pub end: usize,
}

pub fn find_blocks(text: &str, begin: &str, end: &str) -> Vec<Block> {
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
        let step = step_of(&lines, start, begin).unwrap_or_else(|| {
            panic!(
                "bloc {begin} ligne {} sans commentaire TODO-STEP:<n> dans le bloc de commentaires qui le précède",
                start + 1
            )
        });
        let Some(stop) = (start + 1..lines.len()).find(|&k| lines[k].trim_start().starts_with(end))
        else {
            break;
        };
        blocks.push(Block {
            step,
            blank: is_blank_marker(lines[start]),
            indent,
            body: lines[start + 1..stop]
                .iter()
                .map(|line| line.to_string())
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

fn is_blank_marker(line: &str) -> bool {
    line.trim_end().ends_with(" vide") || line.contains(BLANK_NOTE)
}

fn step_of(lines: &[&str], start: usize, begin: &str) -> Option<u8> {
    if begin == REGION_BEGIN {
        return lines[start]
            .trim_start()
            .strip_prefix(REGION_BEGIN)?
            .split_whitespace()
            .next()?
            .parse()
            .ok();
    }
    (0..start)
        .rev()
        .take_while(|&k| lines[k].trim_start().starts_with("//"))
        .find_map(|k| {
            let (_, rest) = lines[k].split_once("TODO-STEP:")?;
            rest.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
}

pub fn rewrite(
    text: &str,
    blocks: &[Block],
    mut replacement: impl FnMut(&Block) -> Option<Vec<String>>,
) -> (String, usize) {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut changed = 0;
    let mut i = 0;

    while i < lines.len() {
        match blocks.iter().find(|block| block.start == i) {
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

fn todo_line(indent: &str, step: u8) -> String {
    format!("{indent}todo!(\"étape {step} — voir le commentaire ci-dessus\")")
}

pub fn hole_body(block: &Block) -> Vec<String> {
    if block.blank {
        Vec::new()
    } else {
        vec![todo_line(&block.indent, block.step)]
    }
}

pub fn is_empty(block: &Block) -> bool {
    block.body.iter().any(|line| line.contains("todo!("))
        || block.body.iter().all(|line| line.trim().is_empty())
}
