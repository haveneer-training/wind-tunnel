# Étape 7 — Ordre 2 en espace : moindres carrés + limiteur

**Fichiers :** `src/gradient.rs` · `src/flux.rs` · **Vérification :** `cargo xtask goto 7`, `cargo test` · **≈ 40 min**

L'étape 6 a mesuré la fausse diffusion du décentrement amont : nulle quand l'écoulement
suit les axes du maillage, maximale à 45°. Sur un maillage non structuré, l'écoulement
n'est *jamais* aligné avec les faces — cette étape s'attaque donc au défaut, pas à un cas
particulier.

`Centered`, déjà dans `flux.rs`, est bien d'ordre 2, et pourtant inutilisable : le champ
y déborde de `[0, 1]` et diverge à mesure que le calcul avance (étape 6, section « Pour
aller plus loin »). La leçon à en tirer n'est pas qu'un schéma d'ordre 2 serait un mauvais
calcul, mais qu'il lui manque une précaution. Cette étape ajoute cette précaution.

## L'idée

Un schéma décentré n'a que deux valeurs à sa disposition : celle de la cellule amont,
retenue telle quelle sur toute la face. Un schéma d'ordre 2 l'extrapole plutôt jusqu'à la
face, à partir d'un gradient reconstruit dans cette cellule :

```text
c_face = c_amont + ∇c_amont · (x_face − x_amont)
```

Reste à obtenir `∇c` sur un maillage où les voisins ne sont ni alignés sur des axes, ni
équidistants, ni même en nombre fixe. La méthode retenue, la reconstruction par
**moindres carrés**, cherche le gradient qui explique le mieux, cellule par cellule,
l'écart de `c` observé vers chacun des centroïdes voisins — un système linéaire 2×2, à
résoudre une fois par cellule et par pas de temps.

Extrapoler sans précaution reproduit exactement le défaut de `Centered` : rien n'empêche
la valeur reconstruite à une face de dépasser les valeurs des cellules qui l'entourent.
Le **limiteur de Barth–Jespersen** rabote chaque gradient, après coup, jusqu'à ce que
toutes ses extrapolations restent dans l'intervalle `[min, max]` des voisins — un facteur
`φ ∈ [0, 1]` par cellule : `φ = 1` ne change rien, `φ = 0` retombe sur l'ordre 1 (une
cellule qui est elle-même un extremum local n'a rien de sûr à extrapoler).

Le nouveau schéma, `Muscl`, n'extrapole que la cellule *amont* — comme `Upwind`, pas
comme `Centered` qui moyenne les deux côtés. C'est ce qui le garde décentré, donc borné.

## Socle

| Fonction | Fichier | Ce qu'elle doit faire |
|---|---|---|
| `least_squares_gradient` | `gradient.rs` | assembler le système normal 2×2 pondéré sur les voisins intérieurs d'une cellule, et le résoudre |
| `Muscl::interface_value` | `flux.rs` | extrapoler la valeur amont jusqu'à la face à partir de son gradient limité ; sans voisin intérieur de ce côté (un bord), retenir la valeur telle quelle |

Le limiteur de Barth–Jespersen (`barth_jespersen`, dans `gradient.rs`) est déjà écrit :
relisez-le, c'est lui qui garantit que `Muscl` reste borné là où `Centered` déborde.

`FaceState` (`flux.rs`) porte désormais, en plus de `c_left`/`c_right`/`un`, le gradient
limité de chaque côté et le déplacement du centroïde jusqu'au milieu de la face —
`grad_right` est `None` sur un bord, où il n'y a rien à extrapoler.

Trois tests unitaires dans `gradient.rs` vérifient la reconstruction : exacte sur un
champ affine (propriété des moindres carrés, quelle que soit l'irrégularité du
maillage), nulle sur un champ uniforme, et bornée par le limiteur autour d'un pic
artificiel. Deux autres dans `flux.rs` vérifient `Muscl` directement sur un `FaceState`
construit à la main.

## Ce qu'il y a à remarquer

```shell
cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --scheme muscl
```

Le champ reste dans `[0, 1]` — comme `Upwind`, contrairement à `Centered` — et les
interfaces des bandes de fumée restent plus nettes plus longtemps. C'est la
reconstruction d'ordre 2 qui travaille, sous la garde du limiteur.

**Et pourtant, si vous relancez `tests/order.rs` avec `Muscl` à la place d'`Upwind`,
l'ordre mesuré ne bouge pas : il reste ≈ 1.** Ce n'est pas un bug — c'est exactement
l'énigme que l'étape 8 explique et résout.

## Pour aller plus loin

- `residual`, dans `solver.rs`, alloue un tampon de gradients à chaque appel — donc à
  chaque pas de temps. C'est la seule allocation qui reste dans la boucle en temps :
  `work`, lui, est alloué une fois par [`Solver::run`] et réutilisé d'un pas à l'autre.
  Faites de même pour les gradients : ajoutez un paramètre à `residual` et `step`, et
  allouez le tampon dans `run`. Mesurez la différence avec `--steps` élevé.
- Comparez le coût de la répartition dynamique et statique. `main.rs` choisit le schéma
  à l'exécution via `Box<dyn FluxScheme>` : chaque appel à `interface_value` passe par
  une table virtuelle. Le solveur, lui, est générique (`Solver<F: FluxScheme>`) :
  appelez-le directement avec `Muscl` plutôt qu'avec la boîte, et chronométrez les deux
  avec `std::time::Instant` sur un grand maillage (`--refine 8` ou plus). La différence
  se mesure-t-elle vraiment ? À quel point du code se trouve l'appel qui compte ?
- Reprenez le cas de l'étape 6 qui isolait la fausse diffusion :
  ```shell
  cargo run --release -- domains/tunnel-empty.dom --refine 4 --bands 9 --angle 45 --scheme upwind
  cargo run --release -- domains/tunnel-empty.dom --refine 4 --bands 9 --angle 45 --scheme muscl
  ```
  Comparez la fraction de cellules à valeur intermédiaire après 480 pas. L'ordre 2 ne
  supprime pas la fausse diffusion — mais il en réduit nettement l'ampleur.
- Désactivez le limiteur (`phi` fixé à `1.0` dans `limited_gradients`) et relancez sur le
  cas avec obstacle. Combien de pas avant que le champ ne sorte de `[0, 1]` ?
