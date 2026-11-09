# Solutions

Ce répertoire contient ce que le corrigé ne peut pas contenir : les réponses en code aux
« pour aller plus loin » des énoncés, et d'autres façons d'écrire les trous.

**Il n'est pas livré aux stagiaires** : `cargo xtrain start` ne le copie pas dans
`travail/`. Il se distribue à part, en fin de formation, ou se consulte avec le formateur.

## Pourquoi un répertoire à part

Le corrigé (le dépôt lui-même) est la solution des trous : chaque bloc
`// SOLUTION-BEGIN` / `// SOLUTION-END` est retiré par `cargo xtrain start` et remis par
`cargo xtrain solve <n>`. Ce mécanisme ne connaît qu'**une** réponse par trou, et ne sait
rien d'un code qui n'est pas un trou. Deux choses lui échappent donc :

- **les extensions** (« réécrivez ce rendu avec `std::thread::spawn` », « écrivez un
  gradient conjugué ») : il n'y a rien à retirer du squelette, le stagiaire écrit du code
  *en plus*. Placé dans `src/`, ce code partirait tel quel dans `travail/`, et alourdirait
  le corrigé de fonctions que le programme n'utilise pas ;
- **les variantes d'un même trou** : la boucle `for` et la chaîne d'itérateurs, la pile
  `Vec` et la file `VecDeque`. Un bloc `SOLUTION` n'en porte qu'une.

Ici, tout est **compilé et testé contre le corrigé** : une réponse rédigée seulement dans
un `.md` dériverait sans que personne le voie.

## Lancer

```shell
cargo test --manifest-path solutions/Cargo.toml
cargo clippy --manifest-path solutions/Cargo.toml --all-targets -- -D warnings
cargo run --release --manifest-path solutions/Cargo.toml --example render_timing
cargo run --release --manifest-path solutions/Cargo.toml --example stream_solvers
```

C'est un **paquet autonome**, avec son propre workspace, son `Cargo.lock` et son
`target/` — comme `solutions/m4-legacy-refactored` dans sentinelle. Il ne peut pas être
membre du workspace racine : `cargo xtrain start` recopie le `Cargo.toml` racine dans
`travail/` sans y livrer `solutions/`, et un membre absent y ferait échouer toute commande
cargo. Le prix : le corrigé est compilé une fois de plus, et rust-analyzer ne voit ce
répertoire que s'il est ouvert comme un projet à part.

Il dépend du corrigé par son chemin (`wind-tunnel = { path = ".." }`), avec ses features
par défaut : la sentinelle `step13` ouvre tout le fil rouge.

## Ce qui vient du corrigé, ce qui est écrit ici

Une réponse ne recopie pas le corrigé : elle **réutilise** ce qu'il exporte, et n'écrit
que ce qui change. Pour cela, quelques éléments internes du corrigé sont publics :

| Exporté par le corrigé | Pour |
|---|---|
| `io::png::Raster` (`new`, `height`, `cell_pixels`) et `io::png::BACKGROUND` | la géométrie du rendu : les variantes de l'étape 10 ne changent que l'organisation des threads |
| `stream::{Adjacency, adjacency, boundary_values, fix_group, obstacle_level, residual_inf, scale_of}` | le système de l'étape 12 : les autres méthodes de résolution travaillent sur exactement les mêmes équations |

La seule duplication est **voulue** : celle de `simple.rs`, dont chaque fonction est une
autre écriture d'un trou du corrigé.

## Contenu

| Fichier | Étape | Ce qu'il contient | Vérifié par |
|---|---|---|---|
| `src/simple.rs` | 0, 2, 5, 6 | les variantes « simples » des trous : boucles `for` et indices au lieu d'adaptateurs d'itérateur (enseignés au J3), quatre `if` au lieu de `checked_sub` et d'`Option` dans `fluid_neighbours`, une pile `Vec` au lieu d'une `VecDeque` dans `explore` | résultat identique au corrigé, au bit près |
| `src/render.rs` | 10 | le rendu PNG avec `std::thread::spawn` et `Arc` (`render_spawn_arc`), sans verrou par le canal (`render_lock_free`), avec `rayon` et un `Mutex` (`render_rayon`) | image identique à `write_png`, au pixel près |
| `src/stream.rs` | 12 | le gradient conjugué *matrix-free*, le SOR, la constante d'obstacle à circulation nulle par superposition | résidu du corrigé (`residual_of`) sous la tolérance ; circulation nulle |
| `examples/render_timing.rs` | 10 | avec ou sans verrou : qui gagne, à quelle taille | — |
| `examples/stream_solvers.rs` | 12 | Jacobi, gradient conjugué et SOR à trois raffinements ; constante de l'obstacle | — |

Les réponses rédigées (explications, mesures, pseudo-code) sont dans les énoncés, à la
fin de chacun, sous **« Éléments de réponse »**, dans des blocs repliés :

| Énoncé | Réponses |
|---|---|
| `docs/etapes/etape-05.md` | la CFL avec diffusion ; pourquoi le schéma centré explose ; le coût de `--refine 4` |
| `docs/etapes/etape-10.md` | mesures avec et sans verrou ; pourquoi des threads « à la main » |
| `docs/etapes/etape-11.md` | l'image assemblée sur le rang 0 ; le recouvrement calcul/communication ; la limite de l'accélération ; MPI contre `rayon` — en pseudo-code, sans code compilé (voir plus bas) |
| `docs/etapes/etape-12.md` | mesures des trois méthodes ; pourquoi Jacobi est lent ; ce que `faer` n'apprend pas ; la circulation nulle ; plusieurs obstacles ; le gradient conjugué distribué ; vers `ψ`–`ω` |

Les énoncés, eux, **sont** livrés : ces réponses rédigées sont visibles des stagiaires,
repliées, comme l'était déjà la réponse de l'étape 2.

## Pas encore couvert

- **Étape 11 en code** : les extensions MPI demandent de modifier le pilote
  (`mpi/src/`), dont les fonctions sont privées à un binaire. Il faudrait ici une copie
  modifiée du pilote, membre d'aucun workspace par défaut pour ne pas exiger MPI.
- **Les autres étapes** : leurs « pour aller plus loin » n'ont pas encore de réponse,
  sauf l'étape 2 (dans l'énoncé) et les étapes 7 et 8 (dans
  `docs/BONUS-OPTIMISATION.md`).
