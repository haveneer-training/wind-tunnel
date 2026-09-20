# État du projet et reprise du travail

Document de passation. Il dit où en est le fil rouge, ce qui reste à faire, et les
décisions qu'il ne faut pas défaire sans le savoir. À tenir à jour.

Dernière mise à jour : 2026-09-20 (rééquilibrage des trous vers le Rust écrit).

## Contexte

Projet fil rouge de la formation « Découverte du langage Rust pour le calcul
scientifique » (ONERA, 3 jours, ≤ 8 participants, proposition PF-202603-001). Le
programme annonce « un projet fil rouge orienté maillage et/ou numérique » ; c'est celui-ci.

Le déroulé pédagogique complet est dans [`ETAPES.md`](../ETAPES.md), les énoncés dans
[`etapes/`](etapes/). Le plan initial, plus détaillé sur les intentions, est resté hors
dépôt : `~/.claude/plans/pour-la-formation-rust-happy-seahorse.md`.

## Ce qui est fait

**Étapes 0 à 12** : géométrie, masque, maillage non structuré et connectivité, sorties
VTK/PNG et erreurs typées, écoulement porteur, solveur explicite (le noyau, étapes 0 à 5),
conservation et ordre de convergence mesurés (étape 6), reconstruction d'ordre 2 par
moindres carrés et limiteur de Barth–Jespersen (étape 7, schéma `Muscl`), RK2 qui fait
enfin apparaître l'ordre 2 mesuré (étape 8), la parallélisation `rayon` des trois
boucles en *gather* du solveur (étape 9), puis le rendu PNG parallélisé « à la main »
avec `thread::scope`, `Mutex` et `mpsc` (étape 10), et enfin la décomposition de domaine
MPI (étape 11, bonus), et l'écoulement calculé sur le maillage (étape 12, bonus). Le code
tourne et produit l'animation.

**Rééquilibrage 2026-09-20.** Le découpage des trous a été révisé pour que le temps du
stagiaire achète du Rust plutôt que de la transcription de formules. Mesuré trou par
trou, le rendement s'effondrait au milieu du parcours (étapes 4, 7 et 8 : 90 min pour une
recopie d'algèbre) pendant que trois concepts annoncés dans le tableau d'`ETAPES.md`
n'étaient jamais écrits — `error.rs` n'avait aucun trou, `Mask::parse` était donnée en
entier, et l'étape 4 « traits, généricité » ne faisait écrire qu'une formule. Ce qui a
changé, à budget constant (355 min sur les étapes 0 à 10) :

| # | Avant | Après |
|---|---|---|
| 1 | `is_fluid` (4 L) + BFS en extension | `is_fluid` + **`parse_row`** : `match ch`, `Err(InvalidChar)`, `Option<usize>`, `&mut Vec` · 30 → 40 min |
| 2 | `build_faces` en extension | l'**appariement** passe au socle (`boundary_face` donnée), extension = **`build_cell_faces`** (CSR) · 40 → 45 min |
| 3 | `write_dataset` entière (34 L) | **`write_cells`** seule (`write_points`/`write_cell_data` données) + **`From<io::Error>`** et **`Error::source`** · 30 → 20 min |
| 4 | `PotentialCylinder::at` (formule) | **`impl VelocityField for Uniform`** et l'**implémentation couvrante `StreamSource`** (`?Sized`) ; le cylindre est donné · 25 → 20 min |
| 7 | assemblage + résolution du système 2×2 | **résolution seule** (`normal_system` donnée) · 40 → 30 min |
| 8, 10 | — | durées seulement : 25 → 20 min, et 30 → **45 min** (71 lignes de `thread::scope`/`Mutex`/`mpsc` ne tenaient pas en 30) |

Trois tests ont été ajoutés à l'étape 4 (`the_uniform_flow_is_constant_everywhere`,
`the_uniform_stream_function_matches_its_velocity`,
`any_velocity_field_is_already_a_stream_source`, ce dernier passant aussi par un
`Box<dyn VelocityField>` pour couvrir le `?Sized`).

- 88 tests verts, `cargo clippy --all-targets --all-features -- -D warnings` propre,
  `cargo fmt` appliqué
- 36 trous répartis : 4 en étape 0, 3 en 1, 3 en 2, 4 en 3, 3 en 4, 3 en 5, 1 en 6, 2 en 7,
  1 en 8, 3 en 9, 1 en 10, 5 en 11, 3 en 12
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
    MPI (`cargo build -p wind-tunnel-mpi` pour le bâtir). Trois trous : `Halo::exchange`
    (réceptions immédiates postées avant les envois, `wait_all`), `global_dt_max`
    (`all_reduce` MIN) et `time_loop`. Les tampons d'échange vivent dans la structure
    `Halo`, créée une fois par rang : c'est pour cela qu'elle traverse `time_loop` et le
    rapporteur en `&mut` au lieu d'être capturée.
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

## Étape 12 (bonus) : l'écoulement calculé

`src/stream.rs` résout `∇²ψ = 0` **aux sommets** du maillage, par balayages de Jacobi
parallélisés (`rayon`), avec Dirichlet sur l'entrée, les parois et l'obstacle, et Neumann
homogène — c'est-à-dire *rien à assembler* — sur la sortie. Le laplacien est celui du
maillage dual : chaque face interne `(a, b)` est une arête de poids
`w = face.distance / face.length`, stockée en CSR, jamais sous forme de matrice.

Les deux approximations annoncées disparaissent : les parois portent une valeur de `ψ`
constante, donc leur débit est **exactement** `0.0` (`assert_eq!`, pas une tolérance) et
`Bc::NoFlux` devient légitime ; et la forme réellement dessinée est respectée, `domains/square.dom`
en fait la démonstration. Ce qui reste hors de portée du modèle : c'est toujours un
écoulement potentiel — pas de couche limite, pas de sillage (voir Pièges).

Trois choix structurants :

- **`StreamSource`** (dans `src/velocity.rs`) est le contrat réellement consommé par
  `compute_face_flux` : `ψ` à un sommet. Une implémentation générale
  `impl<F: VelocityField + ?Sized> StreamSource for F` fait que tout écoulement analytique
  le satisfait déjà, et le `?Sized` est ce qui permet à `Solver::new<S: StreamSource + ?Sized>`
  d'accepter les vingt sites d'appel existants **sans en modifier un seul**. `ComputedStream`
  n'implémente pas `VelocityField` : il ne saurait pas répondre en un point quelconque.
  `app::Carrier` fait l'aiguillage entre les deux — un `&dyn VelocityField` ne pouvant pas
  être converti en `&dyn StreamSource`, c'est l'énumération qui porte l'impl.
- **La constante de `ψ` sur l'obstacle est arbitraire** : on prend `ψ₀` de l'ordonnée
  moyenne de son contour. Exact pour une forme symétrique en `y`, approché sinon. La bonne
  condition (circulation nulle, par superposition de deux résolutions) est en extension.
- **Jacobi est volontairement lent** : 7 134 balayages à `--refine 1`, 23 559 à
  `--refine 2`, 75 221 à `--refine 4` sur `domains/tunnel.dom` (90 780 sommets, quelques
  minutes) — mesuré, et dans le budget par défaut de 200 000. C'est le ressort de
  l'extension (gradient conjugué matrix-free) et de `examples/stream_faer.rs`, qui résout
  le même système par Cholesky creuse avec le crate `faer` : 0,02 s contre 11,6 s, à
  `--refine 2`. `faer` est une dépendance **optionnelle** — `cargo test` ne la compile
  jamais, seul `--features faer` l'active.

Non disponible sous MPI : `mpi/src/main.rs` refuse `--flow computed` avec un message
explicite, aucun rang ne détenant le maillage complet. Le gradient conjugué distribué (le
`Halo` existe déjà) est proposé en « pour aller plus loin ».

## Lire les sorties : ψ, vitesse et date dans les fichiers VTK

Comparer deux modèles d'écoulement à l'œil, sur le seul traceur, ne marche pas : les deux
images se ressemblent, et les critères qui les distinguent (portée de la perturbation,
blocage, coins) demandent de savoir quoi regarder. Trois ajouts y répondent.

- **`Solver::velocity_at(CellId)`** reconstruit la vitesse moyenne d'une cellule à partir
  des seuls débits de face, par l'identité `∫ u dA = ∮ (u·n)(x − x_c) dl`, exacte pour un
  champ uniforme et d'ordre 2 sinon. Elle ne passe pas par `VelocityField::at`, donc elle
  marche à l'identique pour l'écoulement calculé, qui n'en a pas.
- **`vtk::write_frame`** écrit, en plus de `c` : `u` et `speed` aux cellules, `psi` aux
  **sommets**, et la date de l'image. `psi` est le vrai apport — un filtre *Contour* dans
  ParaView en tire les lignes de courant exactes, isolignes et non trajectoires intégrées.
- **`--max-time <s>` et `--every-dt <s>`** expriment en temps physique ce que `--max-steps` et
  `--every` expriment en pas. Ils existent parce que deux modèles n'ont pas le même pas de
  temps — la CFL le déduit du débit maximal, et l'écoulement calculé accélère davantage
  dans les passages : 2,53e-2 s contre 3,98e-2 s sur `square.dom` à `--refine 2`. À
  `--max-steps` égal, les deux calculs ne s'arrêtent donc pas au même instant, et leurs images
  de même rang ne montrent pas la même chose. `--max-time` plafonne la durée (`Config::max_time` ; le
  calcul s'arrête au premier des deux plafonds atteint), `--every-dt` la cadence
  (`Config::output_dt`) ; les deux vivent dans `Solver::run`, au premier pas atteignant la
  date visée et sans dérive cumulée. Le pilote MPI reprend `--max-time` — converti en
  nombre de pas à côté du `dt` global, hors du trou de l'étape 11, les deux plafonds se
  ramenant alors au plus petit — mais refuse `--every-dt`.

`app::caps` porte la règle qui rend ces deux plafonds utilisables : **un défaut ne
contraint jamais une valeur explicitement demandée.** `--max-time 60` donné seul ne subit
donc aucun plafond en pas — sans quoi il s'arrêterait au bout des 600 pas par défaut, qui
ne font pas 60 secondes, et le calcul mentirait sur sa propre durée. Les 600 pas ne servent
que lorsqu'on n'a rien demandé. Un test couvre les quatre combinaisons.

Deux contraintes de structure ont guidé la mise en œuvre, et ne sont pas à défaire :

- Le corps de `write_vtk` **était le trou de l'étape 3**. Il est devenu `write_dataset`,
  de contenu identique mais écrivant dans un `&mut impl Write` ; `write_vtk` et
  `write_frame` l'enveloppent, hors trou. Ajouter `psi` — qui n'existe qu'à l'étape 4 —
  dans le trou aurait rendu l'étape 3 incompréhensible. Le test correspondant est
  d'ailleurs conditionné à `step4`. Depuis le rééquilibrage, `write_dataset` est elle-même
  hors trou : elle appelle `write_points`, `write_cells` et `write_cell_data`, dont seule
  la deuxième est trouée — les deux autres servent de modèle de format.
- Le pilote MPI **refuse `--every-dt`** : la cadence est mise en œuvre dans `Solver::run`,
  que ce pilote n'utilise pas — il a sa propre boucle, celle des halos, et c'est le trou de
  l'étape 11. `--max-time`, lui, passe : il se convertit en nombre de pas dès que le `dt`
  global est connu, et la boucle n'a rien à savoir.

Le temps dans ParaView passe par **`frames.vtk.series`**, écrit en fin de calcul par
`vtk::write_series` : un petit JSON qui donne la date de chaque image. Les deux autres
voies sont des impasses, l'une et l'autre vérifiées contre ParaView 6.1.1 avec `pvpython`
(`/Applications/ParaView-6.1.1.app/Contents/bin/pvpython`) :

- le `.pvd`, le plus connu, fait planter `vtkPVDReader` sur du VTK legacy ;
- la date inscrite dans le fichier, en `FIELD FieldData`, est bien lue mais **n'est pas**
  prise comme axe du temps : sans le `.series`, ParaView numérote les images 0, 1, 2…

Cette date reste écrite (VisIt s'en sert, et elle documente le fichier), mais **sa place est
imposée : entre `DATASET` et `POINTS`**. Écrite après `POINT_DATA`, le lecteur legacy la
rattache aux points — « Point array TIME with 1 components, only has 1 tuples but there are
N points », puis « Attribute Mismatch » — et ParaView refuse le fichier entier, zéro cellule
lue. C'est arrivé, un test verrouille désormais l'ordre.

`--every-dt` reste utile indépendamment du lecteur : il donne aux images des deux calculs
les mêmes dates, à un pas de temps près.

## Ce qui reste

| # | Étape | État de préparation |
|---|---|---|
| 13 | *Bonus* : écoulement à sillage, en variables `ψ`–`ω` | `∇²ψ = −ω` réutilise tel quel le solveur de l'étape 12, et le transport de `ω` réutilise la boucle en temps. Donnerait enfin décollement, sillage et traînée — le piège « écoulement potentiel » ci-dessous. Coût réel : les conditions de paroi sur `ω` et la stabilité en fonction du Reynolds. `step13` est le nom réservé (c'est la sentinelle actuelle). |

`docs/BONUS-OPTIMISATION.md` est un bonus **transversal**, déjà écrit : pas de feature, pas
de trou, pas de `LAST_STEP` à bouger. Il reprend les « pour aller plus loin » des étapes 7
et 8 (tampons de gradients, de RK2) plus la construction du maillage, chiffres et
solutions dépliables à l'appui, et s'appuie sur `examples/alloc_count.rs` — un allocateur
global compteur qui donne le tableau des allocations par phase.

Hors étapes : les slides de transition dans `index.html` du dépôt de formation (point
resté ouvert dans son `TODO-ONERA.md` §3), et le rebranchement éventuel de la démo
interop existante sur la sortie du fil rouge — **démo seulement**, le programme est
explicite là-dessus.

## Avertissements dans `travail/` : la convention `cfg_attr`

Un trou (`todo!()`) rend inutilisés les paramètres de sa fonction, parfois un import ou
une fonction auxiliaire. Sans précaution, un stagiaire à l'étape 0 voyait **50
avertissements**, presque tous à propos d'étapes dont il n'avait pas encore entendu
parler. Chaque cas connu porte donc un `cfg_attr` conditionné à la feature de l'étape :

```rust
#[cfg_attr(not(feature = "step3"), allow(unused_variables))] // trou étape 3
pub fn write_vtk(path: impl AsRef<Path>, mesh: &Mesh, fields: &[(&str, &Field)]) -> ...
```

Deux conditions seulement, et le choix entre les deux se fait sur une question : *cet
avertissement aide-t-il le stagiaire qui travaille sur cette étape ?*

| Cas | Condition | Effet |
|---|---|---|
| L'avertissement **désigne ce qu'il reste à écrire** (paramètre inutilisé de la fonction trouée, import à utiliser) | `not(feature = "stepN")` | muet avant l'étape N, visible **pendant** l'étape N où il sert d'aide-mémoire, muet après (le code est écrit) |
| L'avertissement est **collatéral** : il porte sur du code que le stagiaire ne touche pas (`dead_code` sur une fonction auxiliaire, champ jamais lu) | `not(feature = "stepN+1")` | muet jusqu'à ce que le trou de l'étape N soit rempli, donc jamais vu |

Le commentaire de fin de ligne (`// trou étape 3`, `// collatéral du trou étape 9`) est
obligatoire : sans lui, personne ne retrouve à quel trou l'attribut se rapporte.

Dans le corrigé, `default = ["step12"]` rend **toutes** ces conditions fausses : les
attributs y sont inertes, et rien n'y est caché. Idem dans un `travail/` entièrement
résolu.

### La feature sentinelle

`step12` est la clé de voûte : c'est l'étape *suivant* la dernière implémentée
(`LAST_STEP` vaut 11, et `cargo xtask goto 12` est refusé). Aucun code n'en dépend, elle
n'ouvre aucun test — elle existe pour que la condition `not(feature = "stepN+1")` de la
seconde ligne du tableau ait un sens quand `N` est la dernière étape. Sans elle, le
collatéral du trou de l'étape 11 (`Halo`, `mpi/src/exchange.rs`, dont les quatre tampons
ne sont lus que par le corps troué) n'aurait aucune condition à laquelle se rattacher.

Elle est activée par défaut dans les deux manifestes du corrigé, et retirée du dossier
de travail :

- `Cargo.toml` racine : `default = ["step12"]`. `cargo xtask goto <n>` réécrit cette
  ligne, donc `travail/` ne l'a jamais.
- `mpi/Cargo.toml` : `default = ["step12"]`, avec `step12 = ["wind-tunnel/step12"]` — le
  crate `mpi` n'a pas d'étapes à lui, il lui faut sa propre déclaration pour pouvoir
  écrire `#[cfg(feature = ...)]`. Comme `goto` ne touche pas ce manifeste-là,
  `make_starter` y remet `default = []` au moment d'engendrer `travail/`
  (`rewrite_default`, dans `xtask/src/main.rs`).

Vérification que la sentinelle ne cache rien : ajouter un champ jamais lu à `Halo` dans
le corrigé doit produire « field is never read » malgré l'attribut. C'est le test à
refaire si l'on touche à ce mécanisme.

Quand l'avertissement ne vient pas d'un trou mais d'un `#[cfg]` — un import qui ne sert
qu'à partir d'une certaine étape —, on **conditionne l'import** plutôt que de le taire :

```rust
#[cfg(feature = "step7")]
use crate::geom::Vec2;
```

Résultat : 252 avertissements sur les douze premières étapes, réduits à 47, tous rattachés à
l'étape en cours. Le scan se refait en régénérant `travail/` puis, pour chaque `n` :
`cargo xtask goto n`, `cargo check --all-targets`, `cargo xtask solve n`.

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
6. **Éteindre les avertissements que le trou provoque**, avec le `cfg_attr` conditionné
   à la feature décrit plus haut (§ « Avertissements dans `travail/` »). Un trou crée en
   général deux à quatre `unused_variables`, parfois un `unused_imports` ou un
   `dead_code` sur une fonction auxiliaire devenue orpheline. Se relire après coup avec
   le scan par étape : un avertissement qui apparaît **avant** l'étape concernée est un
   attribut manquant.
   **Si l'étape ajoutée devient la dernière**, déplacer la sentinelle : déclarer
   `step{N+1} = ["stepN"]` dans `Cargo.toml` et y mettre `default = ["step{N+1}"]`,
   sans quoi les `not(feature = "step{N+1}")` du collatéral resteraient vrais dans le
   corrigé et y cacheraient de vrais avertissements.
7. Régénérer et vérifier : `cargo xtask starter --force && cd travail && cargo test`.

## Décisions à ne pas défaire

**Le trou porte sur le Rust, pas sur la formule.** L'objet de la formation est le
langage ; recopier une expression déjà imprimée dans l'énoncé n'en apprend rien. Quand
une fonction mêle du Rust et de la recopie numérique, la recopie est **extraite dans une
fonction auxiliaire donnée** et le trou couvre ce qui reste. C'est ce qui a été fait pour
`normal_system` (étape 7, l'assemblage du système normal), `boundary_face` (étape 2, la
géométrie d'une face), `write_points`/`write_cell_data` (étape 3, le format déjà montré
en exemple) ; et c'est pourquoi `PotentialCylinder::at` est *donnée* alors que
`impl VelocityField for Uniform` est *trouée* (étape 4) : la première est une formule, la
seconde est une implémentation de trait. Conséquence pratique : ces auxiliaires sont
appelées depuis un trou, donc mortes tant qu'il est vide — elles portent un
`#[cfg_attr(not(feature = "stepN+1"), allow(dead_code))]`, **et une `struct` auxiliaire
doit le porter elle aussi** (le `dead_code` des champs jamais lus est attribué au type,
pas à la fonction ; `NormalSystem` s'est fait prendre).

**Les débits de face viennent d'une différence de fonction de courant**, pas d'un
échantillonnage de la vitesse aux faces. C'est ce qui rend la divergence discrète nulle
à la précision machine, donc le schéma décentré borné. La première version
échantillonnait : un champ initialement dans [0, 1] montait à 1,78. Deux tests
verrouillent la propriété.

**La conservation ne dépend pas de la qualité de l'écoulement.** Le débit d'une face
étant `ψ(b) − ψ(a)`, le bilan d'une cellule fermée télescope : il est exactement nul pour
*n'importe quel* champ `ψ` aux sommets, convergé ou non, physique ou non. C'est ce qui
rend l'étape 12 sans danger pour les deux tests à 1e-12, et c'est le meilleur contenu
pédagogique du projet : la propriété vient de la forme du schéma, pas du modèle.

**Les parois sont en `ZeroGradient`, pas en `NoFlux`** — *avec l'écoulement analytique*.
Une paroi en escalier n'en est pas exactement une ligne de courant ; supprimer le petit
débit résiduel fabriquerait une divergence artificielle et ferait perdre la borne. Avec
`--flow computed` (étape 12), ce débit est nul par construction et `app::solver_config`
bascule alors `Wall` et `Obstacle` en `NoFlux`. Ne pas généraliser ce basculement à
l'analytique.

**Indices typés plutôt que pointeurs** (`CellId`, `FaceId`, `VertexId`) et **`Side`
plutôt qu'un `Option` doublé d'un drapeau** : ce sont des supports de discours autant
que des choix techniques.

**La boucle est en *gather***, pas en *scatter* : c'est ce qui a rendu l'étape 9
possible sans réécriture (juste `.iter()` → `.par_iter()`), et c'est le piège C/OpenMP
qu'on exhibe.

**`VtkF64` (`src/io/vtk.rs`) écrit les dénormaux `0`.** Ce n'est pas de la
cosmétique : `vtkDataReader` lit ses nombres par `istream >> double`, qui pose `failbit`
au sous-débordement (`strtod` renvoie `ERANGE` sous `f64::MIN_POSITIVE`). Le flux reste
en échec, le lecteur abandonne le reste du fichier et prend le nombre suivant pour un
mot-clé : Paraview crache « Error reading ascii data » puis « Unsupported cell attribute
type: 3.5e-323 », et affiche n'importe quoi (`c` monté à 91 sur une plage [0, 1] —
constaté). Le traceur descend sous `1e-308` en quelques dizaines de pas, donc tout calcul
un peu long est concerné. Mesuré au passage : la *longueur* du jeton, elle, n'y est pour
rien — un nombre de 402 caractères se relit parfaitement ; le passage en `{:e}` hors des
exposants usuels n'est là que pour la taille et la lisibilité des fichiers. Un test
(`tests/io.rs`) verrouille la propriété. C'est un type à `Display` et non une fonction
renvoyant une `String` : un fichier contient de l'ordre du million de nombres, et une
`String` par nombre faisait 273 691 allocations par fichier contre 3 aujourd'hui.

## Pièges connus, non corrigés

- **L'écoulement analytique ne voit pas la forme dessinée** : `obstacle()` dans
  `src/app.rs` en déduit un disque de même aire, et `PotentialCylinder` utilise ce disque.
  Dessinez un carré, la fumée contournera un cercle. Corrigé depuis l'étape 12 par
  `--flow computed` (démonstration : `domains/square.dom`), mais c'est toujours le
  comportement **par défaut** — donc toujours un piège si on l'ignore, et toujours la
  bonne image de slide pour justifier l'étape 12.
- **Gradient nul sur une frontière d'entrée = problème mal posé.** Visible avec
  `--angle 45` sur `domains/tunnel-empty.dom` : la paroi basse devient une entrée, le
  traceur est réinjecté et la masse augmente de 10 %. Signalé dans l'énoncé 5.
- **Écoulement potentiel** : pas de couche limite, pas de décollement, pas de sillage,
  pas de traînée. Un public CFD le verra en trois secondes — l'annoncer. L'étape 12 n'y
  change rien : elle rend l'écoulement *cohérent avec la géométrie*, pas visqueux. C'est
  l'étape 13 (`ψ`–`ω`) qui s'y attaquerait.
- **Le résidu n'est pas l'erreur** (étape 12) : pour Jacobi, le premier vaut environ
  `(1 − ρ)` fois la seconde, et `1 − ρ` se dégrade comme le carré du raffinement. Avec la
  tolérance par défaut (1e-6), `ψ` est juste à ~1e-1 près sur une échelle de 48 à
  `--refine 2` — invisible à l'œil, mais à savoir avant d'en tirer un chiffre.
- `travail/` n'est pas versionné (ignoré, exclu du workspace). Le participant peut y
  faire son propre `git init`.
- `cargo clippy -p xtask --all-targets -- -D warnings` signale un `useless_format`
  (message d'erreur de `make_starter`). Antérieur, sans rapport avec les étapes : la
  vérification documentée ne couvre que le paquet racine et `mpi`, jamais `xtask`.

## Vérifier que tout va bien

```shell
cargo test                                   # 83 tests
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy -p wind-tunnel-mpi --all-targets -- -D warnings   # nécessite MPI
cargo fmt --all --check
cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --max-steps 960

# le corrigé doit être sans avertissement à *chaque* niveau de feature : c'est ce qui
# prouve que les `cfg_attr` conditionnels n'y cachent rien
for n in $(seq 0 13); do cargo check --no-default-features --features "step$n" --all-targets; done

cargo xtask starter --force                  # régénère travail/
cd travail && cargo test                     # 4 tests rouges : étape 0
cargo xtask goto 12 && cargo xtask solve 12  # ... et tout doit redevenir vert

cd .. && scripts/check-steps.sh              # le même tour, automatisé, étape par étape
scripts/check-mpi.sh                         # étape 11 ; sans effet si mpirun est absent
cargo run --release --features faer --example stream_faer       # extension de l'étape 12
```
