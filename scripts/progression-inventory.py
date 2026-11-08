#!/usr/bin/env python3
"""Inventorie les blocs d'exercice et leur volume corrigé / stagiaire."""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
SOURCE_DIRS = (ROOT / "src", ROOT / "mpi" / "src", ROOT / "design" / "src")
BEGIN = "// SOLUTION-BEGIN"
END = "// SOLUTION-END"
TODO_STEP = re.compile(r"TODO-STEP:(\d+)")
DEFINITION = re.compile(r"^\s*(?:(?:pub(?:\([^)]*\))?)\s+)?(fn|struct|enum|trait|type)\s+(\w+)")


def exercise_name(lines, marker):
    for line in reversed(lines[max(0, marker - 60) : marker]):
        match = DEFINITION.match(line)
        if match:
            return f"{match.group(1)} `{match.group(2)}`"
    return "définitions à écrire"


def inventory_file(path):
    lines = path.read_text(encoding="utf-8").splitlines()
    index = 0
    while index < len(lines):
        if not lines[index].lstrip().startswith(BEGIN):
            index += 1
            continue
        start = index
        stop = next(
            (line for line in range(start + 1, len(lines)) if lines[line].lstrip().startswith(END)),
            None,
        )
        if stop is None:
            raise ValueError(f"{path.relative_to(ROOT)}:{start + 1}: marqueur fermant manquant")
        step = None
        comment = start - 1
        while comment >= 0 and lines[comment].lstrip().startswith("//"):
            match = TODO_STEP.search(lines[comment])
            if match:
                step = int(match.group(1))
                break
            comment -= 1
        if step is None:
            raise ValueError(f"{path.relative_to(ROOT)}:{start + 1}: TODO-STEP absent")
        body = lines[start + 1 : stop]
        reference_lines = sum(bool(line.strip()) for line in body)
        blank = "vide" in lines[start].lower()
        yield {
            "step": step,
            "path": path.relative_to(ROOT).as_posix(),
            "line": start + 1,
            "name": exercise_name(lines, start),
            "reference": reference_lines,
            "starter": 0 if blank else 1,
        }
        index = stop + 1


def main():
    exercises = sorted(
        (exercise for directory in SOURCE_DIRS for path in directory.rglob("*.rs") for exercise in inventory_file(path)),
        key=lambda exercise: (exercise["step"], exercise["path"], exercise["line"]),
    )
    print("| Étape | Blocs | Corrigé (lignes non vides) | Squelette (lignes) |")
    print("|---:|---:|---:|---:|")
    for step in sorted({exercise["step"] for exercise in exercises}):
        rows = [exercise for exercise in exercises if exercise["step"] == step]
        print(
            f"| {step} | {len(rows)} | {sum(row['reference'] for row in rows)} | "
            f"{sum(row['starter'] for row in rows)} |"
        )
    print("\n| Étape | Fichier:ligne | Fonction / bloc | Corrigé → stagiaire |")
    print("|---:|---|---|---:|")
    for row in exercises:
        print(
            f"| {row['step']} | `{row['path']}:{row['line']}` | {row['name']} | "
            f"{row['reference']} → {row['starter']} |"
        )


if __name__ == "__main__":
    main()