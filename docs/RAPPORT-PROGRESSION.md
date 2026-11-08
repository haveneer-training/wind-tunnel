# Rapport de complexité et de progression des exercices

**État au 7 novembre 2026.** Ce rapport décrit le parcours de `wind-tunnel` après la
modularisation de `xtask` et le découpage des exercices en blocs plus ciblés.
L'inventaire détaillé se régénère avec `python3 scripts/progression-inventory.py`.

## Synthèse

Le parcours comprend **65 blocs** répartis sur treize étapes. Le corrigé contient
**513 lignes non vides** dans ces blocs ; le dossier stagiaire reçoit **64 `todo!()`**
d'une ligne et un trou de conception vide. Celui-ci est le bonus isolé `design/` : ses
84 lignes de solution ne deviennent volontairement pas une fonction à compléter, mais
des définitions (`struct`, `enum`, `impl`) à concevoir contre les tests.

Le volume médian est volontairement ramené à de petites unités : dans les étapes 0 à 12,
64 blocs sur 65 s'ouvrent en une seule ligne de squelette, et les nouveaux morceaux
répartissent le travail entre une opération locale, son orchestration et les tests. La
taille du corrigé n'est toutefois pas un indicateur suffisant de difficulté : l'échange
MPI et les conditions de bord demandent plus de raisonnement que leur nombre de lignes ne
le suggère.

## Progression par étape

Les lignes de corrigé comptent les lignes non vides entre les marqueurs. Le « squelette »
compte les lignes de `todo!()` ; le trou de conception compte zéro, car il ne contient pas
de `todo!()`. Les estimations de temps sont celles d'`ETAPES.md`, pas une durée individuelle
garantie.

| Étape | Blocs | Corrigé → squelette | Complexité | Progression / notions dominantes |
|---:|---:|---:|---|---|
| 0 | 5 | 19 → 5 | Faible | Fonctions géométriques, tuples, `Vec`, premières boucles et tests. |
| 1 | 5 | 49 → 5 | Faible à moyenne | Indexation et propriété ; le parcours de connexité est le premier algorithme complet. |
| 2 | 10 | 149 → 9 | Moyenne ; bonus élevé | Forme des cellules, clé canonique et appariement ; extension CSR. Le volume inclut le bonus de conception vide (84 lignes de corrigé). |
| 3 | 10 | 59 → 10 | Moyenne | Erreurs typées, `Option`/`Result`, conversion de caractères séparée de la validation de ligne, écriture VTK. |
| 4 | 3 | 3 → 3 | Faible | Implémentations de traits : le travail est dans le contrat, pas dans la formule. |
| 5 | 4 | 24 → 4 | Moyenne | Débits, CFL et mise à jour du champ ; le chemin du calcul devient exécutable. |
| 6 | 1 | 9 → 1 | Faible à moyenne | Mesure d'erreur pondérée et vérification de propriété numérique. |
| 7 | 2 | 16 → 2 | Moyenne | Reconstruction de gradient et schéma MUSCL, avec les cas dégénérés explicités. |
| 8 | 2 | 16 → 2 | Moyenne | Tampons et `zip` pour RK2 ; comparaison de convergence avant/après. |
| 9 | 3 | 24 → 3 | Élevée | Parallélisation *gather* ; le calcul d'une cellule est fourni, l'attention porte sur les emprunts et les écritures disjointes. |
| 10 | 5 | 31 → 5 | Élevée | Rendu PNG : pixels, cellules et chunks, puis orchestration concurrente avec `scope`/`Mutex`/`mpsc`. |
| 11 | 9 | 60 → 9 | Très élevée · bonus | Décomposition et possession, halos, réduction MPI, boucle temporelle ; raisonnement local puis cohérence entre rangs. |
| 12 | 6 | 54 → 6 | Élevée · bonus | Conditions de bord, balayage Jacobi, tampons alternés et convergence ; le CSR est déjà connu. |
| **Total** | **65** | **513 → 64** | — | **355 min** annoncées pour les étapes 0–10, puis 100 min de bonus pour les étapes 11–12. |

### Lecture du parcours

- **Étapes 0–3 — construire les données (J1–J2).** La charge passe des fonctions courtes
  et de la manipulation de `Vec` aux invariants du maillage, puis à l'exhaustivité des
  erreurs. L'étape 2 a un embranchement facultatif de conception, plus ouvert et plus
  difficile que les trous guidés ; l'étape 3 découpe désormais la conversion locale
  (`cell_of`) de la validation globale (`parse_row`).
- **Étapes 4–8 — faire tourner et vérifier le calcul (J2–J3).** Les étapes 4 et 6 sont
  courtes en lignes mais conceptuelles (traits, conservation et ordre). Les étapes 7 et 8
  font écrire la partie Rust — reconstruction et gestion des tampons — plutôt que de
  recopier les formules déjà isolées dans des helpers.
- **Étapes 9–10 — parallélisme en mémoire partagée (J3).** La difficulté augmente malgré
  un volume de corrigé modéré : l'étape 9 conserve une écriture propre à chaque cellule ;
  l'étape 10 traite l'image partagée en séparant le calcul des pixels de leur écriture
  verrouillée. `Raster` évite de répéter le code numérique et garde les allocations hors
  de la boucle cellule par cellule.
- **Étapes 11–12 — extensions bonus.** L'étape 11 est la plus exigeante : elle ajoute un
  modèle de propriété distribuée et des échanges ordonnés, donc son coût vient surtout
  des cas limites et des invariants inter-rangs. L'étape 12 est un algorithme itératif
  complet, rendu moins opaque par six fonctions courtes et l'adjacence CSR fournie.

## Détail des blocs

`corrigé → stagiaire` indique les lignes de solution non vides remplacées par le squelette.
Les positions pointent sur le marqueur `SOLUTION-BEGIN` ; elles sont recalculées par le
script d'inventaire.

| Étape | Bloc (`fichier:ligne`) | Corrigé → stagiaire |
|---:|---|---:|
| 0 | `src/geom.rs:61` · `dot` | 1 → 1 |
| 0 | `src/geom.rs:83` · `perp_cw` | 1 → 1 |
| 0 | `src/geom.rs:170` · `polygon_area` | 3 → 1 |
| 0 | `src/geom.rs:190` · `polygon_centroid` | 9 → 1 |
| 0 | `src/geom.rs:207` · `vertex_mean` | 5 → 1 |
| 1 | `src/mask.rs:94` · `is_fluid` | 4 → 1 |
| 1 | `src/mask.rs:139` · `refine` | 11 → 1 |
| 1 | `src/mask.rs:204` · `check_connected` | 12 → 1 |
| 1 | `src/mask.rs:225` · `fluid_neighbours` | 12 → 1 |
| 1 | `src/mask.rs:246` · `explore` | 10 → 1 |
| 2 | `design/src/lib.rs:31` · définitions à concevoir | 84 → 0 |
| 2 | `src/mesh.rs:434` · `build_cell_faces` | 5 → 1 |
| 2 | `src/mesh.rs:449` · `face_counts` | 8 → 1 |
| 2 | `src/mesh.rs:466` · `offsets_from_counts` | 5 → 1 |
| 2 | `src/mesh.rs:480` · `fill_cell_faces` | 10 → 1 |
| 2 | `src/mesh.rs:499` · `place_face` | 3 → 1 |
| 2 | `src/mesh.rs:530` · `cell_corners` | 7 → 1 |
| 2 | `src/mesh.rs:567` · `edge_key` | 5 → 1 |
| 2 | `src/mesh.rs:584` · `build_faces` | 10 → 1 |
| 2 | `src/mesh.rs:610` · `pair_edge` | 12 → 1 |
| 3 | `src/error.rs:137` · `source` | 4 → 1 |
| 3 | `src/error.rs:156` · `from` | 1 → 1 |
| 3 | `src/io/png.rs:164` · `contains` | 9 → 1 |
| 3 | `src/io/png.rs:182` · `crosses` | 5 → 1 |
| 3 | `src/io/vtk.rs:108` · `write_cells` | 9 → 1 |
| 3 | `src/io/vtk.rs:126` · `cells_size` | 4 → 1 |
| 3 | `src/io/vtk.rs:139` · `write_cell_line` | 6 → 1 |
| 3 | `src/mask.rs:316` · `cell_of` | 5 → 1 |
| 3 | `src/mask.rs:331` · `check_width` | 10 → 1 |
| 3 | `src/mask.rs:356` · `parse_row` | 6 → 1 |
| 4 | `src/velocity.rs:73` · `stream_at` | 1 → 1 |
| 4 | `src/velocity.rs:90` · `at` | 1 → 1 |
| 4 | `src/velocity.rs:99` · `stream` | 1 → 1 |
| 5 | `src/flux.rs:64` · `interface_value` | 5 → 1 |
| 5 | `src/solver.rs:414` · `step_euler` | 4 → 1 |
| 5 | `src/solver.rs:554` · `max_stable_dt` | 8 → 1 |
| 5 | `src/solver.rs:571` · `cell_transport` | 7 → 1 |
| 6 | `src/field.rs:104` · `mean_abs_error` | 9 → 1 |
| 7 | `src/flux.rs:104` · `interface_value` | 8 → 1 |
| 7 | `src/gradient.rs:52` · `least_squares_gradient` | 8 → 1 |
| 8 | `src/solver.rs:428` · `step_rk2` | 8 → 1 |
| 8 | `src/solver.rs:586` · `heun_combine` | 8 → 1 |
| 9 | `src/gradient.rs:160` · `limited_gradients` | 9 → 1 |
| 9 | `src/solver.rs:318` · `residual` | 8 → 1 |
| 9 | `src/solver.rs:504` · `compute_face_flux` | 7 → 1 |
| 10 | `src/io/png.rs:233` · `render_chunk` | 7 → 1 |
| 10 | `src/io/png.rs:258` · `render_cell` | 9 → 1 |
| 10 | `src/io/png.rs:277` · `chunk_size` | 1 → 1 |
| 10 | `src/io/png.rs:287` · `paint` | 4 → 1 |
| 10 | `src/io/png.rs:307` · `render_all` | 10 → 1 |
| 11 | `mpi/src/exchange.rs:74` · `pack_all` | 2 → 1 |
| 11 | `mpi/src/exchange.rs:91` · `transfer` | 13 → 1 |
| 11 | `mpi/src/exchange.rs:112` · `unpack_all` | 2 → 1 |
| 11 | `mpi/src/exchange.rs:128` · `global_dt_max` | 3 → 1 |
| 11 | `mpi/src/main.rs:77` · `time_loop` | 7 → 1 |
| 11 | `src/decomposition.rs:116` · `owned` | 5 → 1 |
| 11 | `src/decomposition.rs:190` · `new` | 11 → 1 |
| 11 | `src/decomposition.rs:255` · `place` | 9 → 1 |
| 11 | `src/decomposition.rs:273` · `mark_sends` | 8 → 1 |
| 12 | `src/stream.rs:147` · `obstacle_level` | 10 → 1 |
| 12 | `src/stream.rs:169` · `boundary_values` | 9 → 1 |
| 12 | `src/stream.rs:192` · `fix_group` | 7 → 1 |
| 12 | `src/stream.rs:227` · `sweep` | 3 → 1 |
| 12 | `src/stream.rs:239` · `weighted_mean` | 12 → 1 |
| 12 | `src/stream.rs:271` · `jacobi` | 13 → 1 |

## Points de vigilance et critères de maintien

- Les blocs guidés les plus denses — `transfer` (13 lignes), `check_connected`,
  `fluid_neighbours`, `explore` et `weighted_mean` (12) — portent un algorithme ou un
  protocole dont les invariants sont l'objet de l'exercice. Le bonus de conception reste
  distinct avec ses 84 lignes de solution ; ses définitions sont à concevoir contre les
  tests plutôt qu'à recopier.
- Le volume court ne doit pas masquer les prérequis : aucun exercice obligatoire avant
  l'étape 3 ne requiert `Result`, `Option` ou un `enum` d'erreur. Les helpers fournis sont
  conditionnés pour que les étapes précédentes restent compilables et exécutables.
- `cargo xtask next` et l'analyse stricte de CLI complètent `start`, `update`, `status`,
  `goto`, `solve` et `reset`. `LAST_STEP` est défini dans `xtask/src/steps.rs` ; la feature
  `step13` est la sentinelle, et `scripts/check-steps.sh` la déduit du manifeste.
- `cargo xtask update` apparie les trous par `(étape, portée, rang)`, la portée étant le
  chemin `mod`/`impl`/`trait`/`fn` qui les contient (xtask identique à celui de
  sentinelle) : ajouter, retirer ou déplacer un trou dans une autre fonction ne décale pas
  les autres. Renommer une fonction qui contient un trou rempli fait échouer `update` —
  bruyamment, sans rien toucher — plutôt que d'égarer la réponse.
- Vérification complète attendue : `cargo test`, `cargo test -p xtask`,
  `cargo clippy --all-targets --all-features --no-deps -- -D warnings`, `cargo fmt --all --check`,
  `scripts/check-steps.sh`, puis le contrôle MPI si `mpirun` est disponible.