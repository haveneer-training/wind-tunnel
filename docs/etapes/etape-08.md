# Étape 8 — RK2 : pourquoi l'ordre n'avait pas bougé

**Fichiers :** `src/solver.rs` · **Vérification :** `cargo xtask goto 8`, `cargo test` · **≈ 20 min**

À l'étape 6, l'ordre mesuré (`tests/order.rs`) valait ≈ 1 : normal, décentrement amont et
Euler explicite sont tous deux d'ordre 1. À l'étape 7, la reconstruction MUSCL a passé
l'espace à l'ordre 2 — et l'ordre mesuré n'a **pas bougé**. Cette étape referme
l'énigme.

## L'explication

`tests/order.rs` fixe le nombre de Courant, pas le nombre de pas : le pas de temps `dt`
est déduit de la CFL à chaque raffinement, donc `dt ∝ h`. Deux erreurs vivent dans le
résultat numérique — celle du schéma spatial, et celle de l'intégration en temps — et
c'est toujours la moins bonne des deux qui domine la mesure. Passer l'espace à l'ordre 2
sans toucher au temps ne change rien tant que le temps reste le facteur limitant, à
l'ordre 1.

## L'idée

Euler explicite n'évalue le résidu qu'une fois par pas, à l'instant de départ. La
méthode de **Heun** (Runge-Kutta d'ordre 2) en évalue deux : une fois au départ, une
fois en un point prédit à l'arrivée, puis fait la moyenne des deux pentes.

```text
k1 = résidu(c)
prédicteur = c + dt · k1
k2 = résidu(prédicteur)
c ← c + dt/2 · (k1 + k2)
```

Deux résidus par pas au lieu d'un : le coût double, en échange d'un ordre en temps qui
passe de 1 à 2.

<details>
<summary>D'où viennent les noms <code>k1</code> et <code>k2</code> — le tableau de Butcher</summary>

Les trois quantités du calcul ci-dessus ne sont pas de même espèce, et c'est ce que les
noms cachent :

| nom | ce que c'est | unité |
|---|---|---|
| `k1` | `résidu(c)` — la **pente** au début du pas | champ / seconde |
| `prédicteur` | `c + dt·k1` — un **état**, l'estimation d'Euler de l'arrivée | comme `c` |
| `k2` | `résidu(prédicteur)` — la **pente** estimée en fin de pas | champ / seconde |

`prédicteur` dit ce que la chose *est* ; `k1` et `k2` disent seulement *où ils tombent
dans le tableau de Butcher*. Rien dans le type ne les distingue — les trois sont des
`Field`.

Le tableau de Butcher (John C. Butcher, 1963) est la notation compacte qui décrit
n'importe quelle méthode de Runge-Kutta. Pour `y' = f(t, y)`, une méthode explicite à `s`
étages s'écrit :

```text
k_i     = f(t_n + c_i·h,  y_n + h·Σ_{j<i} a_ij·k_j)
y_{n+1} = y_n + h·Σ_i b_i·k_i
```

Trois jeux de coefficients : `c` (quand on évalue), `A` (avec quel mélange des pentes
déjà connues), `b` (comment on les recombine). On les range ainsi :

```text
c │ A
──┼───
  │ b
```

Les trois méthodes les plus courantes :

```text
Euler              Heun (Rk2)              RK4 classique

0 │                0   │                   0   │
──┼───             1   │ 1                 1/2 │ 1/2
  │ 1              ────┼─────────          1/2 │ 0    1/2
                       │ 1/2  1/2          1   │ 0    0    1
                                           ────┼───────────────────
                                               │ 1/6  1/3  1/3  1/6
```

Heun se relit alors ligne à ligne sur le calcul de `step_rk2` :

| tableau | code |
|---|---|
| `c₁ = 0`, ligne vide → `k₁ = f(y_n)` | `k1 = résidu(c)` |
| `c₂ = 1`, `a₂₁ = 1` → évaluer en `y_n + h·k₁` | `prédicteur = c + dt·k1` |
| `k₂ = f(…)` | `k2 = résidu(prédicteur)` |
| `b = (1/2, 1/2)` | `c ← c + dt/2 · (k1 + k2)` |

Deux remarques :

- **La colonne `c` est inerte ici.** L'écoulement est stationnaire, le résidu ne dépend
  pas explicitement du temps — c'est pourquoi `Solver::residual(c, out)` n'a pas
  d'argument temps. Les `c_i` ne serviraient que pour un terme source variable en temps.
- **Le nombre d'étages est le nombre d'évaluations du résidu par pas** : Euler 1, Heun 2,
  RK4 4. C'est tout le coût de la méthode, et en MPI (étape 11) c'est aussi le nombre
  d'échanges de halo par pas de temps.

</details>

## Socle

| Fonction | Fichier | Ce qu'elle doit faire |
|---|---|---|
| `Solver::step_rk2` | `solver.rs` | le calcul ci-dessus : `k1`, prédicteur, `k2`, puis la mise à jour de `c` |

`TimeScheme` (`Euler` par défaut, `Rk2`) est un nouveau champ de `Config`, déjà branché
sur `--time-scheme euler\|rk2` en CLI et sur `Solver::step`, qui bascule vers
`step_euler` (inchangée depuis l'étape 5) ou `step_rk2` selon le schéma choisi — rien à
modifier de ce côté.

Un nouveau test dans `tests/order.rs`, `order_of_convergence_reaches_two_with_muscl_and_rk2`,
mesure l'ordre avec MUSCL + RK2 exactement comme celui de l'étape 6 mesurait Upwind +
Euler.

## Ce qu'il y a à remarquer

```shell
cargo test order_of_convergence
```

Le premier test reste à ≈ 1 : Upwind + Euler n'a pas changé. Le second, avec MUSCL et
RK2 tous deux d'ordre 2, mesure un ordre proche de **2**. C'est la conclusion de
l'enquête ouverte à l'étape 6 : un schéma spatial d'ordre 2 ne se voit dans le résultat
que si l'intégration en temps ne l'étouffe pas.

```shell
cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --scheme muscl --time-scheme rk2
```

Le champ reste borné — RK2 ne change rien à la propriété de bornage, qui vient du
limiteur de l'étape 7, pas de l'intégration en temps.

## Pour aller plus loin

- `step_rk2` alloue son prédicteur et son second résidu à chaque appel, donc à chaque
  pas de temps — comme `residual` alloue son tampon de gradients depuis l'étape 7.
  Ajoutez les tampons manquants à la signature de `step` et `run` (`Solver::run` en
  possède déjà un, `work`) pour que la boucle en temps n'alloue plus rien, quel que soit
  le schéma temporel. `Field::copy_from` est là pour ça : il recopie dans un tampon
  existant là où `c.clone()` en alloue un neuf. Énoncé complet et solution dans
  [`docs/BONUS-OPTIMISATION.md`](../BONUS-OPTIMISATION.md).
- Chronométrez un grand maillage (`--refine 8` ou plus, `--max-steps` élevé) en Euler puis
  en RK2, avec `std::time::Instant`. Le rapport de temps mesuré est-il proche de 2, comme
  attendu du nombre d'évaluations du résidu ?
- RK2 fait deux fois le travail par pas mais le pas de temps stable (`max_stable_dt`)
  n'a pas changé : il dépend du schéma spatial et de la CFL, pas de l'intégrateur
  temporel. Un schéma d'ordre 2 en temps mais instable au-delà de la même limite CFL
  qu'Euler — est-ce surprenant pour une méthode explicite ?
