//! Ligne de commande : analyse d'argv et aiguillage vers les commandes.

use crate::starter::{make_starter, WORKDIR};
use crate::steps::{fill, goto, next, reset, status, LAST_STEP};
use crate::update::update;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum Command {
    Start { dir: Option<PathBuf>, force: bool },
    Update { dir: Option<PathBuf> },
    Status,
    Goto(u8),
    Next,
    Solve(u8),
    Reset(u8),
    Help,
}

pub fn run(command: Command, root: &Path) -> Result<(), String> {
    let workdir = |dir: Option<PathBuf>| dir.unwrap_or_else(|| root.join(WORKDIR));
    match command {
        Command::Start { dir, force } => make_starter(root, &workdir(dir), force),
        Command::Update { dir } => update(root, &workdir(dir)),
        Command::Status => status(root),
        Command::Goto(step) => goto(root, step),
        Command::Next => next(root),
        Command::Solve(step) => fill(root, step, true),
        Command::Reset(step) => reset(root, step),
        Command::Help => Ok(()),
    }
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some((name, rest)) = args.split_first() else {
        return Ok(Command::Help);
    };
    let unexpected = |arg: &str| format!("argument inattendu « {arg} » pour `{name}`");
    let no_args = |command: Command| match rest.first() {
        Some(arg) => Err(unexpected(arg)),
        None => Ok(command),
    };
    let step = || match rest {
        [] => Err("il manque le numéro d'étape".to_string()),
        [raw] => parse_step(raw),
        [_, arg, ..] => Err(unexpected(arg)),
    };

    match name.as_str() {
        "help" | "-h" | "--help" => no_args(Command::Help),
        "status" => no_args(Command::Status),
        "next" => no_args(Command::Next),
        "goto" => step().map(Command::Goto),
        "solve" => step().map(Command::Solve),
        "reset" => step().map(Command::Reset),
        "start" | "update" => {
            let mut dir = None;
            let mut force = false;
            let mut args = rest.iter();
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--dir" if dir.is_some() => return Err("--dir donné deux fois".to_string()),
                    "--dir" => match args.next() {
                        Some(value) if !value.starts_with("--") => {
                            dir = Some(PathBuf::from(value));
                        }
                        _ => return Err("--dir attend un chemin".to_string()),
                    },
                    "--force" if name == "start" => force = true,
                    _ => return Err(unexpected(arg)),
                }
            }
            if name == "start" {
                Ok(Command::Start { dir, force })
            } else {
                Ok(Command::Update { dir })
            }
        }
        _ => Err(format!("commande inconnue « {name} »")),
    }
}

pub fn usage() -> String {
    format!(
        "Outillage du fil rouge « soufflerie numérique »\n\
         \n\
           cargo xtask status                 où en est le projet, étape par étape\n\
           cargo xtask goto <n>               passer à l'étape n (0 à {LAST_STEP})\n\
           cargo xtask next                   passer à l'étape suivante\n\
           cargo xtask solve <n>              remplir les trous de l'étape n\n\
           cargo xtask reset <n>              rouvrir les trous de l'étape n\n\
           cargo xtask start [--dir <p>]      créer le dossier de travail (par défaut : {WORKDIR}/)\n\
           cargo xtask update [--dir <p>]     y reporter les modifications du corrigé\n\
         \n\
         `start` et `update` s'utilisent depuis le dépôt du corrigé, les autres depuis le dossier de travail.\n"
    )
}

fn parse_step(raw: &str) -> Result<u8, String> {
    let step: u8 = raw
        .parse()
        .map_err(|_| format!("« {raw} » n'est pas un numéro d'étape"))?;
    if step > LAST_STEP {
        return Err(format!(
            "l'étape {step} n'existe pas encore (0 à {LAST_STEP})"
        ));
    }
    Ok(step)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_line(line: &str) -> Result<Command, String> {
        parse(
            &line
                .split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn parse_accepts_every_command() {
        assert_eq!(parse_line(""), Ok(Command::Help));
        assert_eq!(parse_line("--help"), Ok(Command::Help));
        assert_eq!(parse_line("status"), Ok(Command::Status));
        assert_eq!(parse_line("next"), Ok(Command::Next));
        assert_eq!(parse_line("goto 3"), Ok(Command::Goto(3)));
        assert_eq!(parse_line("solve 0"), Ok(Command::Solve(0)));
        assert_eq!(
            parse_line(&format!("reset {LAST_STEP}")),
            Ok(Command::Reset(LAST_STEP))
        );
        assert_eq!(
            parse_line("start"),
            Ok(Command::Start {
                dir: None,
                force: false
            })
        );
        assert_eq!(
            parse_line("start --force --dir p"),
            Ok(Command::Start {
                dir: Some("p".into()),
                force: true
            })
        );
        assert_eq!(
            parse_line("update --dir p"),
            Ok(Command::Update {
                dir: Some("p".into())
            })
        );
    }

    #[test]
    fn parse_rejects_unknown_and_malformed_arguments() {
        for line in [
            "bidule",
            "status x",
            "goto",
            "goto x",
            "goto 13",
            "goto 3 --force",
            "start --dir",
            "start --dir --force",
            "start --dir a --dir b",
            "start --frce",
            "start p",
            "update --force",
        ] {
            assert!(parse_line(line).is_err(), "`{line}` doit être refusé");
        }
    }
}
