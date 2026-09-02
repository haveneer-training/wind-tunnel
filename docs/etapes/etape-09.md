# Étape 9 — Paralléliser avec `rayon`

**Fichiers :** `src/solver.rs`, `src/gradient.rs` · **Vérification :** `cargo xtask goto 9`,
`cargo test` · **≈ 40 min**

Le solveur est écrit en *gather* depuis l'étape 5 : chaque cellule lit ses voisines et
n'écrit que sa propre case. Personne n'écrit chez le voisin — c'est précisément ce qui
rend cette étape possible sans réécrire quoi que ce soit. `FluxScheme` et
`VelocityField` sont même déjà bornés par `Sync` (voir `flux.rs`, `velocity.rs`) : cette
étape ne fait que le payer.

## L'idée

[`rayon`](https://docs.rs/rayon) transforme un itérateur séquentiel en itérateur
parallèle : `.iter()` devient `.par_iter()`, `.iter_mut()` devient `.par_iter_mut()`, et
le reste — `map`, `zip`, `enumerate`, `collect`, `for_each` — s'écrit pareil. Le travail
est découpé en tâches réparties sur un pool de threads (par défaut, un par cœur) ; le
style reste fonctionnel, rien ne change dans la forme du calcul.

Trois boucles du solveur sont de bonnes candidates, dans l'ordre croissant de
difficulté :

1. `Solver::new` calcule le débit de chaque face par un simple `map` — le cas le plus
   direct, seul `.iter()` change.
2. `gradient::limited_gradients` boucle sur les cellules et écrit dans une tranche `out`
   fournie par l'appelant : il faut paralléliser une écriture, pas seulement une
   lecture.
3. `Solver::residual` fait la même chose mais lit en plus `self` (le maillage, les
   débits, le schéma, la config) à chaque itération : le cas le plus complet.

## Le piège à éviter

Le corps de ces boucles ne doit *rien* changer : même lectures, même écritures, juste
réparties sur plusieurs threads. Si le compilateur refuse votre parallélisation, c'est
qu'elle tente d'écrire deux fois au même endroit depuis deux threads — exactement ce que
la forme *scatter*, évoquée dans l'en-tête de `solver.rs`, ferait sans broncher en C ou
en Fortran, et qui donnerait un résultat faux de façon non reproductible. En Rust, ça ne
compile pas.

## Socle

| Bloc | Fichier | Ce qu'il doit devenir |
|---|---|---|
| `face_flux` dans `Solver::new` | `solver.rs` | le même `map`, en `par_iter()` |
| `limited_gradients` | `gradient.rs` | la boucle en `par_iter_mut().enumerate().for_each(...)` |
| la boucle principale de `Solver::residual` | `solver.rs` | `par_iter()` sur les cellules, `zip`é avec `out` en écriture, `enumerate()` pour l'identifiant de cellule |

`Cargo.toml` déclare déjà `rayon` en dépendance ; il ne reste qu'à `use
rayon::prelude::*;` dans chaque fichier concerné.

## Ce qu'il y a à remarquer

```shell
cargo test parallel_execution_matches_sequential_bit_for_bit
```

Ce test (`tests/parallel.rs`) force tour à tour un seul thread puis plusieurs, et
compare les champs obtenus **au bit près**. Rien n'est sommé dans un ordre qui
dépendrait du nombre de threads — chaque cellule écrit sa propre case, indépendamment
des autres — donc le résultat est déterministe, contrairement à ce qu'on pourrait
craindre d'un calcul parallèle.

```shell
RAYON_NUM_THREADS=1 cargo run --release -- domains/tunnel.dom --refine 8 --steps 300
cargo run --release -- domains/tunnel.dom --refine 8 --steps 300
```

`RAYON_NUM_THREADS` fixe la taille du pool de threads de rayon. Chronométrez les deux
(`time cargo run ...`) : l'accélération observée doit se rapprocher du nombre de cœurs
de la machine, sans jamais l'atteindre — la lecture du masque, la construction du
maillage et l'écriture des images restent séquentielles.

## Pour aller plus loin

- `--refine 13` sur `domains/tunnel.dom` donne de l'ordre du million de cellules :
  de quoi rendre la différence Euler/RK2 (étape 8) et séquentiel/parallèle nettement
  visible au chronomètre, plutôt que noyée dans le bruit de mesure.
- Essayez, dans une copie de travail jetable, de réécrire `residual` en *scatter*
  (boucler sur les faces, ajouter le flux aux deux cellules qu'elle touche) puis de le
  paralléliser avec `par_iter()` sur les faces. Le compilateur doit refuser — c'est le
  message d'erreur qui compte ici, pas le code.
- Le pas de temps `dt` est calculé une fois, avant la boucle en temps : `max_stable_dt`
  n'a pas été touché ici. Vaut-il la peine de le paralléliser aussi ? Comparez son coût
  à celui d'un seul appel à `residual`.
