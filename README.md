# Soufflerie numérique

Projet fil rouge de la formation **« Découverte du langage Rust pour le calcul
scientifique »**.

On construit, étape par étape, un petit code de calcul complet : lecture d'un domaine,
génération d'un maillage non structuré, transport d'un traceur passif autour d'un
obstacle, écriture des résultats en VTK et en PNG. Le sujet est modeste, le chemin ne
l'est pas : c'est celui de n'importe quel code de calcul, en réduction.

![filets de fumée déviés par un cylindre](docs/apercu.png)

<sub>`cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --steps 960`</sub>

## Démarrage

Ce dépôt contient le **code complet**. Pour le construire vous-même, engendrez votre
dossier de travail : les passages à écrire y deviennent des `todo!()`, tout le reste est
fourni.

```shell
cargo xtask starter    # crée travail/ (15 trous à combler)
cd travail
cargo test             # quatre tests rouges : l'étape 0 vous attend
```

À partir de là, la boucle est toujours la même :

```shell
cargo test             # rouge : chaque échec nomme le fichier et la ligne
#   ... lire docs/etapes/etape-00.md, remplacer le todo!() entre >>> et <<< ...
cargo test             # vert
cargo xtask goto 1     # étape suivante
```

`cargo test` ne montre que les étapes déjà ouvertes : quatre tests rouges au démarrage,
pas quarante. `cargo xtask status` dit où vous en êtes ; `cargo xtask solve <n>` remplit
une étape à votre place si vous décrochez, `reset <n>` la rouvre. `goto` **n'écrase
jamais** ce que vous avez écrit : il ne complète que les blocs restés vides.

Le déroulé complet est dans [`ETAPES.md`](ETAPES.md), les énoncés dans
[`docs/etapes/`](docs/etapes/) — commencez toujours par lire celui de l'étape en cours.

Et si vous voulez seulement voir tourner la version finie, depuis ce dépôt-ci :

```shell
cargo test                                # tout est vert
cargo run --release -- domains/tunnel.dom # écrit out/frame_XXXX.png et .vtk
open out/frame_0010.png                   # ou paraview out/frame_0010.vtk
```

Quelques options utiles :

```shell
cargo run --release -- domains/tunnel.dom \
    --steps 800 --every 20 \
    --refine 4 \             # subdivise chaque case en 4×4 : le vrai raffinement
    --bands 9 \              # densité du rideau de fumée
    --circulation 4 \        # dissymétrie de l'écoulement (effet Magnus)
    --diffusivity 0.02       # diffusion physique du traceur
```

`--refine` est le seul moyen d'augmenter la résolution : c'est le masque qui fixe le
nombre de cellules. `--h` ne règle que la *taille* d'une cellule, donc la taille physique
du domaine — le diviser par deux redonne exactement la même image, sur un domaine deux
fois plus petit.

Et un second masque, `domains/tunnel-empty.dom`, une veine vide : combiné à `--angle`,
il sert à isoler la diffusion numérique du schéma, sans obstacle pour brouiller la
lecture (voir la fin de l'énoncé de l'étape 5).

`--help` liste le reste.

## Prérequis

- Rust ≥ 1.90 (`rustup update stable`)
- rien d'autre : deux dépendances, `image` pour encoder les PNG et `rayon` pour
  l'étape 9
- pour l'étape 11 seulement, une implémentation de MPI (`brew install open-mpi`,
  `apt install libopenmpi-dev`). Elle n'est utilisée que par le crate `mpi/`, exclu du
  workspace : sans elle, tout le reste se construit et se teste normalement.

En réseau isolé, `cargo vendor` permet de récupérer les dépendances à l'avance.

## Le domaine

`domains/tunnel.dom` est un fichier texte que vous pouvez modifier avec n'importe quel
éditeur : `.` pour du fluide, `#` pour du solide, `%` pour un commentaire.

```text
% une veine et son obstacle
...........
....###....
....###....
...........
```

Le maillage en est déduit : une cellule par case de fluide, et un chanfrein à 45° là où
deux parois perpendiculaires se rejoignent — d'où un maillage réellement mixte,
triangles et quadrangles.

Attention : l'écoulement porteur analytique est celui d'un **cylindre**. Si vous
redessinez l'obstacle, gardez-le rond, ou calculez l'écoulement sur votre géométrie
(étape 12).

## Organisation

```text
masque ASCII ──▶ mask ──▶ mesh ──▶ field ──▶ solver ──▶ io ──▶ PNG / VTK
                                     ▲          ▲
                               velocity       flux
```

| Fichier | Rôle |
|---|---|
| `src/geom.rs` | points, vecteurs, aires, centroïdes |
| `src/mask.rs` | lecture et validation du domaine |
| `src/mesh.rs` | cellules, faces, connectivité |
| `src/field.rs` | un champ scalaire aux cellules |
| `src/velocity.rs` | écoulements porteurs |
| `src/flux.rs` | schémas de flux |
| `src/solver.rs` | boucle en temps, CFL, conditions aux limites |
| `src/io/` | sorties VTK et PNG |
| `src/error.rs` | les deux familles d'erreurs |
| `src/app.rs` | montage d'un cas, partagé par les deux exécutables |
| `src/decomposition.rs` | découpage en bandes pour le calcul distribué (étape 11) |
| `mpi/` | le pilote MPI, crate à part (étape 11) |

Et à côté du code lui-même :

| Chemin | Rôle |
|---|---|
| `ETAPES.md` | le déroulé des étapes |
| `docs/etapes/` | un énoncé par étape |
| `domains/` | les masques de domaine |
| `xtask/` | l'outil qui engendre `travail/` et pilote les étapes |
| `scripts/` | vérifications automatiques du dispositif d'étapes et du pilote MPI |
| `docs/AVANCEMENT.md` | état du projet, ce qui reste, décisions à ne pas défaire |

## Licence

CC BY-NC-SA 4.0 — Pascal Havé (contact@haveneer.com).
