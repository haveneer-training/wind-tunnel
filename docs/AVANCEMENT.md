# État du projet et reprise du travail

Document de passation. Il dit où en est le fil rouge, ce qui reste à faire, et les
décisions qu'il ne faut pas défaire sans le savoir. À tenir à jour.

Dernière mise à jour : 2026-09-02 (étape 10).

## Contexte

Projet fil rouge de la formation « Découverte du langage Rust pour le calcul
scientifique » (ONERA, 3 jours, ≤ 8 participants, proposition PF-202603-001). Le
programme annonce « un projet fil rouge orienté maillage et/ou numérique » ; c'est celui-ci.

Le déroulé pédagogique complet est dans [`ETAPES.md`](../ETAPES.md), les énoncés dans
[`etapes/`](etapes/). Le plan initial, plus détaillé sur les intentions, est resté hors
dépôt : `~/.claude/plans/pour-la-formation-rust-happy-seahorse.md`.

## Ce qui est fait

**Étapes 0 à 11** : géométrie, masque, maillage non structuré et connectivité, sorties
VTK/PNG et erreurs typées, écoulement porteur, solveur explicite (le noyau, étapes 0 à 5),
conservation et ordre de convergence mesurés (étape 6), reconstruction d'ordre 2 par
moindres carrés et limiteur de Barth–Jespersen (étape 7, schéma `Muscl`), RK2 qui fait
enfin apparaître l'ordre 2 mesuré (étape 8), la parallélisation `rayon` des trois
boucles en *gather* du solveur (étape 9), puis le rendu PNG parallélisé « à la main »
avec `thread::scope`, `Mutex` et `mpsc` (étape 10), et enfin la décomposition de domaine
MPI (étape 11, bonus). Le code tourne et produit l'animation.

- 69 tests verts (62 hors doctests), `cargo clippy --all-targets --all-features -- -D warnings` propre,
  `cargo fmt` appliqué
- 27 trous répartis : 4 en étape 0, 2 en 1, 2 en 2, 2 en 3, 1 en 4, 3 en 5, 1 en 6, 2 en 7,
  1 en 8, 3 en 9, 1 en 10, 5 en 11
- **Étape 11 (bonus)** décompose le domaine en **bandes verticales**, une par rang MPI.
  Chaque rang extrait sa tranche de colonnes du masque (`Mask::columns`, nouveau),
  appelle `Mesh::from_mask` dessus et translate le maillage à sa place (`Mesh::translate`,
  nouveau) : **aucun rang ne construit le maillage global**, ce qui était la contrainte
  de départ. Conséquence heureuse : `solver.rs`, `field.rs`, `flux.rs`, `gradient.rs`,
  `mesh.rs` (hors `translate`) et `io/` ne changent pas d'une ligne — le solveur
  travaillait déjà sur « un maillage » sans supposer que c'était tout le domaine.
  - `src/decomposition.rs` (feature `step11`) porte tout ce qui ne parle pas de MPI :
    `Bands` (découpage des colonnes), `Layout` (qui possède quoi, qui envoie quoi à qui),
    `pack`/`unpack`, et les réductions restreintes aux cellules possédées
    (`owned_mass`, `owned_min_max`). Deux trous, testables sans MPI installé.
  - `mpi/` est membre du workspace (`members = ["xtask", "mpi"]`), le seul à dépendre de
    `rsmpi`, mais pas du groupe *par défaut* : un workspace non virtuel ne construit que
    son paquet racine sans `-p`/`--workspace`. `cargo build`, `cargo test` et surtout
    `cargo clippy --all-features` à la racine restent donc verts sur une machine sans
    MPI (`cargo build -p wind-tunnel-mpi` pour le bâtir). Trois trous : `exchange_halo`
    (réceptions immédiates postées avant les envois, `wait_all`), `global_dt_max`
    (`all_reduce` MIN) et `time_loop`.
  - Le pilote écrit **sa propre boucle en temps** au lieu d'appeler `Solver::run` : il
    faut reprendre la main entre les évaluations de résidu pour communiquer (deux
    échanges par pas en RK2, le prédicteur ayant lui aussi des fantômes à rafraîchir).
  - Pas de rassemblement pour la sortie : chaque rang écrit `rankN_frame_XXXX.vtk/.png`
    sur son maillage local, en coordonnées globales. Rapatrier le champ sur le rang 0
    l'obligerait à mailler tout le domaine — le contresens qu'on voulait éviter. Les
    coordonnées globales font que les bandes se recollent bien à l'affichage, mais deux
    nombres varient dans ces noms de fichiers (le rang et le pas de temps) : ParaView
    fonde sa détection de série temporelle sur le seul nombre collé à l'extension, donc
    l'**ordre compte** — `rank` en tête et `frame` juste avant `.vtk` (pas l'inverse) lui
    fait ouvrir une série par rang, chacune animée sur le bon axe, plutôt qu'une série
    par pas de temps avec les rangs mélangés dedans. Essayé et abandonné : un `.pvd`
    explicite (une entrée XML par (pas de temps, rang)) est la solution « propre » sur
    le papier, mais fait planter `vtkPVDReader` de ParaView 6.1.1 sur du VTK legacy
    (`EXC_BAD_ACCESS` dans `vtkXMLCollectionReader::ReadXMLDataImpl`, reproduit avec
    `pvpython`, `part` comme `group`) — ce lecteur attend des fichiers XML
    (`.vtu`/`.vtp`), pas le format legacy que ce projet écrit délibérément.
  - `tests/decomposition.rs` (9 tests) rejoue la décomposition dans un seul processus,
    halos échangés par mémoire, et compare au calcul monolithique cellule par cellule.
    `scripts/check-mpi.sh` lance ensuite le vrai binaire à 1, 2, 3 et 4 rangs et compare
    au binaire séquentiel ; il sort proprement en code 0 si `mpirun` est introuvable.
- **`src/app.rs` (nouveau, sans trou)** : `Args`, `parse_args`, `USAGE`, `obstacle`,
  `velocity_for`, `flux_scheme`, `solver_config`, `initial_field` — tout le montage d'un
  cas, autrefois dans `src/main.rs`, désormais partagé mot pour mot par le binaire
  séquentiel et le binaire MPI. `src/main.rs` n'avait aucun trou : le déplacement ne
  perturbe aucune étape.
- **Le halo fait trois colonnes, et c'est une découverte de cette étape.** Une pour la
  valeur de la voisine, deux parce que MUSCL lit le *gradient* de la voisine, trois parce
  que ce gradient est un moindres carrés sur les **centroïdes** de la deuxième couche —
  et qu'un centroïde dépend du chanfrein de la cellule, donc de ses propres voisines
  (étape 2). *Le maillage lui-même a un stencil.* Mesuré : à deux colonnes, le calcul à
  trois rangs sur `tunnel.dom` s'écarte de 8·10⁻³ du séquentiel, sans rien de faux dans
  la communication. Un obstacle rectangulaire ne le montre pas — il ne produit aucun
  chanfrein —, il faut un coin rentrant, donc `domains/tunnel.dom`. La constante est
  `decomposition::HALO`, documentée sur place.
- **Étape 10** parallélise le rendu PNG (`src/io/png.rs::write_png`), le seul endroit du
  projet où plusieurs unités de travail écrivent dans **la même** structure partagée (une
  `RgbImage`) plutôt que chacune dans sa propre case — le compilateur ne peut pas prouver
  que les cellules n'écrivent jamais le même pixel, donc il refuse tout accès concurrent
  tant qu'il n'est pas protégé. Contrairement à l'étape 9, ce n'est pas `rayon` qui est
  utilisé mais `std::thread::scope` (emprunt de `mesh`/`field`/`Mutex`, pas d'`Arc`
  nécessaire puisque le scope rejoint tous les threads), un `Mutex<RgbImage>` verrouillé
  le temps le plus court possible (le calcul des pixels se fait hors verrou), et un
  `mpsc::channel` cloné par thread pour le suivi — vérifié par un `debug_assert_eq!` qui
  compare la somme reçue à `mesh.n_cells()`. `Arc` est volontairement absent du socle
  (pas nécessaire avec `thread::scope`) ; l'énoncé (`docs/etapes/etape-10.md`) l'explique
  et le fait pratiquer en extension via `std::thread::spawn`, qui lui l'exige. `tests/
  render.rs` verrouille le résultat : deux rendus du même champ doivent être identiques
  au bit près, y compris avec moins de cellules que de threads disponibles
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
- **Correction (2026-09-02) : `cargo run` était cassé de l'étape 5 à l'étape 8.** Le bloc
  `TODO-STEP:9` de `face_flux`/`residual`/`limited_gradients` remplaçait *tout* le calcul
  (version séquentielle comprise) par sa version `rayon` : dans `travail/`, tant que
  l'étape 9 n'était pas atteinte, ce bloc restait un `todo!()`, et `cargo run` paniquait
  dès l'étape 5 malgré ce que dit plus haut « à la fin de l'étape 5, le code tourne ».
  Trouvé en préparant l'étape 10, qui répétait le même défaut sur `write_png`. Corrigé
  en donnant à chaque calcul concerné deux définitions, choisies par les features
  `stepN` déjà présentes (`#[cfg(feature = "step9")]` pour la version `rayon` — avec son
  trou — et `#[cfg(not(feature = "step9"))]` pour la version séquentielle d'avant,
  toujours complète, jamais un trou). Vérifié par un tour complet
  `starter --force` → `goto 3/5/7/8/9/10` → `cargo run` à chaque étape intermédiaire.
  Cette convention est ajoutée à la liste ci-dessous.
- **Correction (2026-09-02, bis) : `residual` avait le même défaut envers l'étape 7.**
  Trouvé par `scripts/check-steps.sh` (nouveau, voir plus bas) : aux étapes 5 et 6,
  `residual` appelait quand même `gradient::limited_gradients`, un calcul de l'étape 7,
  ajouté là par le commit de l'étape 7 sans garder de version d'avant — donc 3 des
  tests de l'étape 5 restaient rouges après `solve 5`. `residual` a maintenant trois
  définitions : `#[cfg(not(feature = "step7"))]` (sans gradient, celle d'avant l'étape 7),
  `#[cfg(all(feature = "step7", not(feature = "step9")))]` (avec gradient, séquentielle),
  `#[cfg(feature = "step9")]` (parallèle, inchangée). Les étapes 7, 9 et 10 sont
  aujourd'hui les cas connus (`residual` ; `face_flux`/`limited_gradients` ;
  `write_png`).
- **`scripts/check-steps.sh`** rejoue `starter --force` puis `goto`/`solve` de 0 à
  `LAST_STEP` dans l'ordre croissant, et échoue si `cargo run` panique sur une étape
  antérieure à celle en cours (avant `solve`) ou déjà résolue (après) — exactement la
  classe de bug ci-dessus. À relancer après toute étape qui touche du code déjà
  fonctionnel plutôt qu'un trou resté vide.

## Ce qui reste

| # | Étape | État de préparation |
|---|---|---|
| 12 | *Bonus* : écoulement calculé (Jacobi puis CG matrix-free) | Supprimerait deux approximations d'un coup : le débit résiduel aux parois, et le fait que l'écoulement ne « voit » qu'un disque équivalent. |

Hors étapes : les slides de transition dans `index.html` du dépôt de formation (point
resté ouvert dans son `TODO-ONERA.md` §3), et le rebranchement éventuel de la démo
interop existante sur la sortie du fil rouge — **démo seulement**, le programme est
explicite là-dessus.

## Conventions à respecter pour ajouter une étape

1. **Un trou** = un bloc encadré de `// SOLUTION-BEGIN` / `// SOLUTION-END`, précédé
   d'un commentaire `// TODO-STEP:<n>` qui porte la consigne. Le bloc doit couvrir un
   corps de fonction entier ou une expression complète : son remplacement par `todo!()`
   doit typecheck.
   **Si l'étape remplace un calcul déjà fonctionnel** (comme l'étape 7 sur `residual`,
   l'étape 9 sur `face_flux`, ou l'étape 10 sur `write_png`) plutôt que de combler un
   trou resté vide depuis le début, donnez-lui deux définitions choisies par
   `#[cfg(feature = "stepN")]` / `#[cfg(not(feature = "stepN"))]` : la version avec le
   trou d'un côté, l'ancienne version (complète, sans `todo!()`) de l'autre — trois
   définitions si l'étape suivante y touche encore (voir `residual`, qui cumule 7 et 9).
   Sinon, dans `travail/`, ce calcul reste un `todo!()` — et donc `cargo run` cassé — de
   l'étape précédente jusqu'à celle-ci, pas seulement pendant celle-ci.
   **`scripts/check-steps.sh`** vérifie automatiquement cette propriété sur toutes les
   étapes ; le relancer après avoir touché du code partagé entre étapes.
   **Le `// TODO-STEP:<n>` doit tenir dans les 8 lignes qui précèdent
   `// SOLUTION-BEGIN`** : `step_of` (`xtask/src/main.rs`) ne remonte pas plus haut, et
   un bloc dont il ne trouve pas le marqueur est silencieusement attribué à l'étape 0.
   Symptôme : `cargo xtask status` compte trop de trous en étape 0 et pas assez dans la
   vôtre. Une consigne plus longue va dans le commentaire de documentation de la
   fonction, que le stagiaire lit de toute façon.
2. **Les tests de l'étape n** vont sous `#[cfg(all(test, feature = "stepN"))]`, ou
   `#![cfg(feature = "stepN")]` en tête d'un fichier de `tests/`. Les features sont
   chaînées dans `Cargo.toml` (`stepN = ["stepN-1"]`), ce qui fait que `cargo test` ne
   montre au stagiaire que les étapes déjà ouvertes.
3. **Relever `LAST_STEP`** dans `xtask/src/main.rs`. Si l'étape ajoute un crate à part,
   l'ajouter aussi à la liste de copie de `make_starter` **et** à `SOURCE_DIRS` — sans
   quoi `starter`/`goto`/`solve`/`reset`/`status` ne verront jamais ses trous (c'est ce
   qu'a demandé `mpi/src` à l'étape 11).
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
résiduel fabriquerait une divergence artificielle et ferait perdre la borne. L'étape 12
supprime le résidu à la source.

**Indices typés plutôt que pointeurs** (`CellId`, `FaceId`, `VertexId`) et **`Side`
plutôt qu'un `Option` doublé d'un drapeau** : ce sont des supports de discours autant
que des choix techniques.

**La boucle est en *gather***, pas en *scatter* : c'est ce qui a rendu l'étape 9
possible sans réécriture (juste `.iter()` → `.par_iter()`), et c'est le piège C/OpenMP
qu'on exhibe.

**`format_f64()` (`src/io/vtk.rs`) écrit les dénormaux `0`.** Ce n'est pas de la
cosmétique : `vtkDataReader` lit ses nombres par `istream >> double`, qui pose `failbit`
au sous-débordement (`strtod` renvoie `ERANGE` sous `f64::MIN_POSITIVE`). Le flux reste
en échec, le lecteur abandonne le reste du fichier et prend le nombre suivant pour un
mot-clé : Paraview crache « Error reading ascii data » puis « Unsupported cell attribute
type: 3.5e-323 », et affiche n'importe quoi (`c` monté à 91 sur une plage [0, 1] —
constaté). Le traceur descend sous `1e-308` en quelques dizaines de pas, donc tout calcul
un peu long est concerné. Mesuré au passage : la *longueur* du jeton, elle, n'y est pour
rien — un nombre de 402 caractères se relit parfaitement ; le passage en `{:e}` hors des
exposants usuels n'est là que pour la taille et la lisibilité des fichiers. Un test
(`tests/io.rs`) verrouille la propriété.

## Pièges connus, non corrigés

- **L'écoulement ne voit pas la forme dessinée** : `obstacle()` dans `src/main.rs` en
  déduit un disque de même aire, et `PotentialCylinder` utilise ce disque. Dessinez un
  carré, la fumée contournera un cercle. C'est une excellente image de slide pour
  justifier l'étape 12, mais c'est un piège si on l'ignore.
- **Gradient nul sur une frontière d'entrée = problème mal posé.** Visible avec
  `--angle 45` sur `domains/tunnel-empty.dom` : la paroi basse devient une entrée, le
  traceur est réinjecté et la masse augmente de 10 %. Signalé dans l'énoncé 5.
- **Écoulement potentiel** : pas de couche limite, pas de décollement, pas de sillage,
  pas de traînée. Un public CFD le verra en trois secondes — l'annoncer.
- `travail/` n'est pas versionné (ignoré, exclu du workspace). Le participant peut y
  faire son propre `git init`.

## Vérifier que tout va bien

```shell
cargo test                                   # 69 tests
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --steps 960

cargo xtask starter --force                  # régénère travail/
cd travail && cargo test                     # 4 tests rouges : étape 0
cargo xtask goto 11 && cargo xtask solve 11  # ... et tout doit redevenir vert

cd .. && scripts/check-steps.sh              # le même tour, automatisé, étape par étape
scripts/check-mpi.sh                         # étape 11 ; sans effet si mpirun est absent
```
