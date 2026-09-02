# État du projet et reprise du travail

Document de passation. Il dit où en est le fil rouge, ce qui reste à faire, et les
décisions qu'il ne faut pas défaire sans le savoir. À tenir à jour.

Dernière mise à jour : 2026-09-02 (étape 9).

## Contexte

Projet fil rouge de la formation « Découverte du langage Rust pour le calcul
scientifique » (ONERA, 3 jours, ≤ 8 participants, proposition PF-202603-001). Le
programme annonce « un projet fil rouge orienté maillage et/ou numérique » ; c'est celui-ci.

Le déroulé pédagogique complet est dans [`ETAPES.md`](../ETAPES.md), les énoncés dans
[`etapes/`](etapes/). Le plan initial, plus détaillé sur les intentions, est resté hors
dépôt : `~/.claude/plans/pour-la-formation-rust-happy-seahorse.md`.

## Ce qui est fait

**Étapes 0 à 9** : géométrie, masque, maillage non structuré et connectivité, sorties
VTK/PNG et erreurs typées, écoulement porteur, solveur explicite (le noyau, étapes 0 à 5),
conservation et ordre de convergence mesurés (étape 6), reconstruction d'ordre 2 par
moindres carrés et limiteur de Barth–Jespersen (étape 7, schéma `Muscl`), RK2 qui fait
enfin apparaître l'ordre 2 mesuré (étape 8), puis la parallélisation `rayon` des trois
boucles en *gather* du solveur (étape 9). Le code tourne et produit l'animation.

- 55 tests verts, `cargo clippy --all-targets --all-features -- -D warnings` propre,
  `cargo fmt` appliqué
- 21 trous répartis : 4 en étape 0, 2 en 1, 2 en 2, 2 en 3, 1 en 4, 3 en 5, 1 en 6, 2 en 7,
  1 en 8, 3 en 9
- l'ordre mesuré à l'étape 6 est **≈ 1** (décentrement amont + Euler explicite, CFL fixe
  donc `dt ∝ h`) — c'est voulu, et l'étape 7 ne le fait **pas** bouger non plus, parce que
  l'erreur en temps domine tant que `dt ∝ h`, quel que soit l'ordre spatial. L'étape 8
  intègre en temps par RK2 (Heun) : l'ordre mesuré passe alors à **≈ 2**
  (`order_of_convergence_reaches_two_with_muscl_and_rk2`, `tests/order.rs`)
- `src/gradient.rs` : `least_squares_gradient` (le trou), `barth_jespersen` (fourni),
  orchestrés par `limited_gradients`, appelée depuis `Solver::residual` — qui alloue donc
  un tampon de gradients à chaque pas de temps ; c'est signalé comme extension dans
  `etapes/etape-07.md` plutôt que résolu dans le socle, pour ne pas alourdir l'étape
- `FaceState` porte désormais `grad_left`/`grad_right`/`to_face_left`/`to_face_right` ;
  `grad_right` est `None` sur un bord. `Muscl` (le second trou) extrapole la seule
  cellule amont — décentré comme `Upwind`, donc borné, contrairement à `Centered`
- `Solver::step` (étape 8) est devenu un dispatcher sur `Config::time_scheme`
  (`TimeScheme::Euler` par défaut, `Rk2`), branché sur `--time-scheme` en CLI. Le corps
  de l'étape 5 est inchangé, déplacé tel quel dans `step_euler` ; `step_rk2` (le trou)
  alloue son prédicteur et son second résidu à chaque appel, comme `residual` alloue son
  tampon de gradients depuis l'étape 7 — même choix, même renvoi en extension
- le dispositif de travail (`cargo xtask starter` puis `goto` / `solve` / `reset` /
  `status`) est en place et vérifié depuis un clone neuf, `LAST_STEP` à jour dans
  `xtask/src/main.rs`
- **Étape 9** parallélise trois boucles en *gather* avec `rayon` : le calcul des débits
  de face dans `Solver::new` (un simple `map`), `gradient::limited_gradients`, et la
  boucle principale de `Solver::residual`. Aucune n'écrit chez le voisin, donc aucune ne
  demande de réécriture — juste `.iter()` → `.par_iter()` (et `.par_iter_mut()` côté
  écriture). `FluxScheme` et `VelocityField` étaient déjà bornés par `Sync` depuis leur
  écriture (étapes 4-5), en prévision de cette étape justement (`velocity.rs` le dit en
  toutes lettres). `tests/parallel.rs` verrouille le résultat : un run forcé à un thread
  et un run à plusieurs threads doivent produire des champs identiques au bit près —
  aucune réduction flottante dont l'ordre dépendrait du nombre de threads
- **Correction : `cargo run` était cassé de l'étape 5 à l'étape 8.** Le bloc
  `TODO-STEP:9` de `face_flux`/`residual`/`limited_gradients` remplaçait *tout* le calcul
  (version séquentielle comprise) par sa version `rayon` : dans `travail/`, tant que
  l'étape 9 n'était pas atteinte, ce bloc restait un `todo!()`, et `cargo run` paniquait
  dès l'étape 5 malgré ce que dit plus haut « à la fin de l'étape 5, le code tourne ».
  Corrigé en donnant à chaque calcul concerné deux définitions, choisies par les
  features `stepN` déjà présentes (`#[cfg(feature = "step9")]` pour la version `rayon`
  — avec son trou — et `#[cfg(not(feature = "step9"))]` pour la version séquentielle
  d'avant, toujours complète, jamais un trou). Vérifié par un tour complet
  `starter --force` → `goto 3/5/7/8/9` → `cargo run` à chaque étape intermédiaire.
  Cette convention est ajoutée à la liste ci-dessous.

## Ce qui reste

| # | Étape | État de préparation |
|---|---|---|
| 10 | Threads : écriture recouverte, `Arc`/`Mutex` | Rien de préparé. Le quiz montre que le groupe connaît le faux partage mais moins les verrous : orienter vers les seconds. |
| 11 | *Bonus* : écoulement calculé (Jacobi puis CG matrix-free) | Supprimerait deux approximations d'un coup : le débit résiduel aux parois, et le fait que l'écoulement ne « voit » qu'un disque équivalent. |
| 12 | *Bonus* : MPI | Crate à part, hors du workspace par défaut, pour ne pas casser `cargo build` sans MPI. |

Hors étapes : les slides de transition dans `index.html` du dépôt de formation (point
resté ouvert dans son `TODO-ONERA.md` §3), et le rebranchement éventuel de la démo
interop existante sur la sortie du fil rouge — **démo seulement**, le programme est
explicite là-dessus.

## Conventions à respecter pour ajouter une étape

1. **Un trou** = un bloc encadré de `// SOLUTION-BEGIN` / `// SOLUTION-END`, précédé
   d'un commentaire `// TODO-STEP:<n>` qui porte la consigne. Le bloc doit couvrir un
   corps de fonction entier ou une expression complète : son remplacement par `todo!()`
   doit typecheck.
   **Si l'étape remplace un calcul déjà fonctionnel** (comme l'étape 9 sur `face_flux`)
   plutôt que de combler un trou resté vide depuis le début, donnez-lui deux
   définitions choisies par `#[cfg(feature = "stepN")]` / `#[cfg(not(feature =
   "stepN"))]` : la version avec le trou d'un côté, l'ancienne version séquentielle
   (complète, sans `todo!()`) de l'autre. Sinon, dans `travail/`, ce calcul reste un
   `todo!()` — et donc `cargo run` cassé — de l'étape précédente jusqu'à celle-ci, pas
   seulement pendant celle-ci.
2. **Les tests de l'étape n** vont sous `#[cfg(all(test, feature = "stepN"))]`, ou
   `#![cfg(feature = "stepN")]` en tête d'un fichier de `tests/`. Les features sont
   chaînées dans `Cargo.toml` (`stepN = ["stepN-1"]`), ce qui fait que `cargo test` ne
   montre au stagiaire que les étapes déjà ouvertes.
3. **Relever `LAST_STEP`** dans `xtask/src/main.rs`.
4. **Un énoncé** `docs/etapes/etape-NN.md`, avec un socle court et une extension
   facultative — l'écart de niveau dans le groupe va de 6 à 9 sur 17 au quiz d'entrée.
5. **Langue** : identifiants et noms de fichiers de code en anglais, prose en français
   (commentaires, doc, messages d'erreur, énoncés). Les noms de tests sont du code.
6. Régénérer et vérifier : `cargo xtask starter --force && cd travail && cargo test`.

## Décisions à ne pas défaire

**Les débits de face viennent d'une différence de fonction de courant**, pas d'un
échantillonnage de la vitesse aux faces. C'est ce qui rend la divergence discrète nulle
à la précision machine, donc le schéma décentré borné. La première version
échantillonnait : un champ initialement dans [0, 1] montait à 1,78. Deux tests
verrouillent la propriété.

**Les parois sont en `ZeroGradient`, pas en `NoFlux`.** Une paroi en escalier n'est pas
exactement une ligne de courant de l'écoulement analytique ; supprimer le petit débit
résiduel fabriquerait une divergence artificielle et ferait perdre la borne. L'étape 11
supprime le résidu à la source.

**Indices typés plutôt que pointeurs** (`CellId`, `FaceId`, `VertexId`) et **`Side`
plutôt qu'un `Option` doublé d'un drapeau** : ce sont des supports de discours autant
que des choix techniques.

**La boucle est en *gather***, pas en *scatter* : c'est ce qui a rendu l'étape 9
possible sans réécriture (juste `.iter()` → `.par_iter()`), et c'est le piège C/OpenMP
qu'on exhibe.

## Pièges connus, non corrigés

- **L'écoulement ne voit pas la forme dessinée** : `obstacle()` dans `src/main.rs` en
  déduit un disque de même aire, et `PotentialCylinder` utilise ce disque. Dessinez un
  carré, la fumée contournera un cercle. C'est une excellente image de slide pour
  justifier l'étape 11, mais c'est un piège si on l'ignore.
- **Gradient nul sur une frontière d'entrée = problème mal posé.** Visible avec
  `--angle 45` sur `domains/tunnel-empty.dom` : la paroi basse devient une entrée, le
  traceur est réinjecté et la masse augmente de 10 %. Signalé dans l'énoncé 5.
- **Écoulement potentiel** : pas de couche limite, pas de décollement, pas de sillage,
  pas de traînée. Un public CFD le verra en trois secondes — l'annoncer.
- `travail/` n'est pas versionné (ignoré, exclu du workspace). Le participant peut y
  faire son propre `git init`.

## Vérifier que tout va bien

```shell
cargo test                                   # 55 tests
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --steps 960

cargo xtask starter --force                  # régénère travail/
cd travail && cargo test                     # 4 tests rouges : étape 0
cargo xtask goto 9 && cargo xtask solve 9    # ... et tout doit redevenir vert
```
