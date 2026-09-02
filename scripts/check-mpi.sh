#!/usr/bin/env bash
#
# Vérifie le pilote MPI de l'étape 11 : le calcul distribué doit donner le même résultat
# que le calcul séquentiel, quel que soit le nombre de rangs.
#
# Le crate `mpi/` n'est pas dans les membres *par défaut* du workspace et n'est bâti que
# par ce script : sur une machine sans MPI, le script s'arrête proprement et ne fait pas
# échouer la vérification globale.
#
#   scripts/check-mpi.sh            # 1, 2, 3 et 4 rangs
#   RANKS="1 2 8" scripts/check-mpi.sh

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

ranks="${RANKS:-1 2 3 4}"
domain=domains/tunnel.dom
run_args=(--refine 2 --bands 3 --steps 20 --every 10 --scheme muscl --time-scheme rk2)

if ! command -v mpirun >/dev/null 2>&1; then
    echo "mpirun est introuvable : étape 11 non vérifiée (ce n'est pas une erreur)."
    echo "Pour l'installer : brew install open-mpi, ou apt install libopenmpi-dev."
    exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "== construction"
cargo build --release --quiet
if ! cargo build --release --quiet -p wind-tunnel-mpi; then
    echo "le crate mpi/ ne se construit pas : étape 11 non vérifiée."
    exit 1
fi

# Ne garde que les lignes de diagnostic, communes aux deux exécutables.
diagnostics() {
    grep -E '^  pas +[0-9]+ ' || true
}

echo "== référence séquentielle"
./target/release/wind-tunnel "$domain" "${run_args[@]}" --out "$work/seq" \
    | diagnostics > "$work/seq.txt"
test -s "$work/seq.txt" || { echo "le calcul séquentiel n'a rien produit"; exit 1; }
cat "$work/seq.txt"

for n in $ranks; do
    echo "== mpirun -n $n"
    mpirun -n "$n" --oversubscribe target/release/wind-tunnel-mpi \
        "$domain" "${run_args[@]}" --out "$work/mpi$n" \
        | diagnostics > "$work/mpi$n.txt"

    python3 - "$work/seq.txt" "$work/mpi$n.txt" "$n" <<'PY'
import re, sys

def read(path):
    rows = []
    for line in open(path, encoding="utf-8"):
        numbers = re.findall(r"-?\d+\.?\d*(?:e[-+]?\d+)?", line)
        rows.append([float(x) for x in numbers])
    return rows

reference, obtained = read(sys.argv[1]), read(sys.argv[2])
ranks = sys.argv[3]
if len(reference) != len(obtained):
    sys.exit(f"{ranks} rangs : {len(obtained)} lignes de diagnostic au lieu de "
             f"{len(reference)}")
for line, (a, b) in enumerate(zip(reference, obtained), 1):
    if len(a) != len(b):
        sys.exit(f"{ranks} rangs, ligne {line} : format inattendu")
    for expected, got in zip(a, b):
        if abs(got - expected) > 1e-6 * max(1.0, abs(expected)):
            sys.exit(f"{ranks} rangs, ligne {line} : {got} au lieu de {expected}")
print(f"  {ranks} rangs : identique au séquentiel")
PY

    frames="$(ls "$work/mpi$n" | grep -c '\.vtk$')"
    expected=$((3 * n))
    test "$frames" -eq "$expected" \
        || { echo "$n rangs : $frames fichiers VTK au lieu de $expected"; exit 1; }
done

echo
echo "Étape 11 vérifiée : le calcul distribué reproduit le calcul séquentiel."
