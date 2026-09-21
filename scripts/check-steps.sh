#!/usr/bin/env bash
# Vérifie le dispositif de travail étape par étape : régénère travail/, puis pour
# chaque étape 0..LAST_STEP (croissant), goto <n> doit compiler et — si un todo!()
# est atteint — paniquer sur une étape >= n, jamais une étape antérieure (c'est
# précisément le bug corrigé par les commits « Étape 9 : corriger cargo run cassé... » /
# « Étape 10 »). solve <n> doit ensuite ramener cargo test au vert et cargo run au
# bout sans panique.
#
# Usage : scripts/check-steps.sh   (depuis n'importe où dans le dépôt)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

last_step="$(sed -n 's/.*LAST_STEP: u8 = \([0-9][0-9]*\);.*/\1/p' xtask/src/main.rs)"
if [ -z "$last_step" ]; then
    echo "!! LAST_STEP introuvable dans xtask/src/main.rs" >&2
    exit 1
fi

domain="$root/domains/tunnel.dom"
run_args=(--refine 2 --bands 3 --max-steps 3)

red=$'\033[31m'
green=$'\033[32m'
reset=$'\033[0m'

fail() {
    echo "${red}!! $*${reset}" >&2
    exit 1
}
ok() {
    echo "   ${green}ok${reset} $*"
}

cleanup() {
    cd "$root"
    rm -rf travail
}
trap cleanup EXIT

echo "==> étapes 0 à ${last_step}"
echo "==> cargo xtask start --force"
cargo xtask start --force >/dev/null
cd "$root/travail"

echo "==> état initial : 4 tests rouges attendus (étape 0)"
set +e
initial_test_out="$(cargo test --lib 2>&1)"
set -e
if echo "$initial_test_out" | grep -q "test result: FAILED"; then
    ok "rouge, comme attendu"
else
    fail "état initial déjà vert — le dossier de travail ne part pas de zéro"
fi

# Lance le solveur sur un tout petit maillage et rapporte soit un succès, soit le
# numéro d'étape du premier todo!() rencontré (0 si le message ne s'y retrouve pas).
run_and_report_step() {
    local out status
    set +e
    out="$(cargo run --release -- "$domain" "${run_args[@]}" 2>&1)"
    status=$?
    set -e
    if [ "$status" -eq 0 ]; then
        echo "success"
        return
    fi
    local n
    n="$(echo "$out" | grep -oE 'étape [0-9]+ — voir le commentaire' | grep -oE '[0-9]+' | head -1 || true)"
    if [ -z "$n" ]; then
        echo "$out" | tail -20 >&2
        fail "cargo run a échoué sans todo!() reconnaissable"
    fi
    echo "$n"
}

for n in $(seq 0 "$last_step"); do
    echo "==> étape ${n}"

    echo "    goto ${n}"
    cargo xtask goto "$n" >/dev/null

    echo "    build"
    cargo build >/dev/null 2>&1 || fail "étape ${n} : cargo build échoue avant même solve"

    echo "    run (todo!() attendu sur l'étape ${n} ou une suivante, jamais avant)"
    reached="$(run_and_report_step)"
    if [ "$reached" = "success" ]; then
        ok "cargo run a réussi (aucun todo!() atteignable sur le chemin d'exécution)"
    elif [ "$reached" -lt "$n" ]; then
        fail "étape ${n} : cargo run panique sur l'étape ${reached}, antérieure — régression du bug étape 9/10"
    else
        ok "todo!() atteint sur l'étape ${reached} (>= ${n})"
    fi

    echo "    solve ${n}"
    cargo xtask solve "$n" >/dev/null

    # --lib --tests seulement : les doctests ne sont pas gérés par les features stepN
    # (voir convention #2 de docs/AVANCEMENT.md) et peuvent viser une fonction d'une
    # étape pas encore atteinte (ex. Mask::refine, étape 1, à l'étape 0).
    echo "    test (doit être entièrement vert, hors doctests)"
    cargo test --lib --tests >/dev/null 2>&1 || fail "étape ${n} : cargo test rouge après solve"
    ok "cargo test vert"

    # goto/solve n ne remplit que les étapes <= n : tant que n < last_step, cargo run
    # peut légitimement buter sur le todo!() d'une étape suivante pas encore atteinte.
    # Ce n'est une vraie régression que si ça bute sur une étape <= n (déjà résolue).
    echo "    run (todo!() attendu sur une étape > ${n}, ou succès à la dernière étape)"
    reached="$(run_and_report_step)"
    if [ "$reached" = "success" ]; then
        if [ "$n" -eq "$last_step" ]; then
            ok "cargo run réussit (dernière étape)"
        else
            ok "cargo run a réussi (aucun todo!() atteignable sur le chemin d'exécution)"
        fi
    elif [ "$reached" -le "$n" ]; then
        fail "étape ${n} : après solve, cargo run panique encore sur l'étape ${reached} — solve a échoué"
    else
        ok "todo!() atteint sur l'étape ${reached} (> ${n}, pas encore résolue)"
    fi
done

# L'exercice de conception (bonus de l'étape 2) vit dans un crate hors du groupe par
# défaut : la boucle ci-dessus ne l'a donc jamais construit. Ses trous ont pourtant été
# remplis par `solve 2`, et son corrigé mérite d'être vérifié comme le reste.
echo "==> bonus conception (design/)"
cargo test -p wind-tunnel-design >/dev/null 2>&1 \
    || fail "design/ : cargo test rouge alors que solve 2 est passé"
ok "cargo test -p wind-tunnel-design vert"

# Et il doit être rouge quand le trou est vide — c'est tout l'exercice. On le rouvre,
# on vérifie que ça ne compile plus, puis on le remplit de nouveau.
cargo xtask reset 2 >/dev/null
if cargo test -p wind-tunnel-design >/dev/null 2>&1; then
    fail "design/ : cargo test vert alors que le trou vient d'être rouvert"
fi
ok "rouge une fois le trou rouvert, comme attendu"
cargo xtask solve 2 >/dev/null

echo
echo "${green}==> étapes 0 à ${last_step} toutes vérifiées${reset}"
