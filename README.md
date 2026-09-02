# Soufflerie numérique

Projet fil rouge de la formation **« Découverte du langage Rust pour le calcul
scientifique »**.

On construit, étape par étape, un petit code de calcul complet : lecture d'un domaine,
génération d'un maillage non structuré, transport d'un traceur passif autour d'un
obstacle, écriture des résultats en VTK et en PNG. Le sujet est modeste, le chemin ne
l'est pas : c'est celui de n'importe quel code de calcul, en réduction.

![filets de fumée déviés par un cylindre](docs/apercu.png)

## Démarrage

```shell
cargo test                                   # tout doit être vert
cargo run --release -- domains/veine.dom     # écrit out/frame_XXXX.png et .vtk
open out/frame_0010.png                      # ou paraview out/frame_0010.vtk
```

Quelques options utiles :

```shell
cargo run --release -- domains/veine.dom \
    --steps 800 --every 20 \
    --circulation 4 \        # dissymétrie de l'écoulement (effet Magnus)
    --diffusivity 0.02 \     # diffusion physique du traceur
    --h 0.5                  # raffinement : cellules deux fois plus petites
```

`--help` liste le reste.

## Prérequis

- Rust ≥ 1.90 (`rustup update stable`)
- rien d'autre : une seule dépendance, `image`, pour encoder les PNG

En réseau isolé, `cargo vendor` permet de récupérer les dépendances à l'avance.

## Le domaine

`domains/veine.dom` est un fichier texte que vous pouvez modifier avec n'importe quel
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
(étape 11).

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

Le déroulé des étapes est dans [`ETAPES.md`](ETAPES.md), les énoncés dans
[`docs/etapes/`](docs/etapes/).

## Licence

CC BY-NC-SA 4.0 — Pascal Havé (contact@haveneer.com).
