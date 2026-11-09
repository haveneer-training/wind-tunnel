#!/usr/bin/env bash
# Contrôles propres à la soufflerie, lancés par `cargo xtrain check-steps` dans le dossier
# de travail qu'il engendre (jamais le travail/ du dépôt) :
#
#   start  rien de plus que les contrôles communs ;
#   end    le bonus de conception (design/, étape 2), hors du groupe par défaut : la boucle
#          des étapes ne l'a jamais construit.
#
# Le reste (marqueurs, toolchain, étapes, cargo run, avertissements) est vérifié par
# xtask-training. Variables reçues : XTRAIN_ROOT, XTRAIN_WORKDIR, XTRAIN_LAST_STEP.
set -euo pipefail

red='' green='' reset=''
if [ -t 1 ]; then
    red=$'\033[31m' green=$'\033[32m' reset=$'\033[0m'
fi
fail() {
    echo "${red}!! $*${reset}" >&2
    exit 1
}
ok() {
    echo "   ${green}ok${reset} $*"
}

case "${1:-}" in
start) ;;
end)
    # Ses trous ont été remplis par `solve 2` : son corrigé se vérifie comme le reste.
    cargo test -p wind-tunnel-design >/dev/null 2>&1 \
        || fail "design/ : cargo test rouge alors que solve 2 est passé"
    ok "cargo test -p wind-tunnel-design vert"
    # Et il doit être rouge quand le trou est vide — c'est tout l'exercice.
    cargo xtrain reset 2 >/dev/null
    if cargo test -p wind-tunnel-design >/dev/null 2>&1; then
        fail "design/ : cargo test vert alors que le trou vient d'être rouvert"
    fi
    ok "rouge une fois le trou rouvert, comme attendu"
    cargo xtrain solve 2 >/dev/null
    ;;
*)
    fail "usage : $0 start|end (lancé par cargo xtrain check-steps)"
    ;;
esac
