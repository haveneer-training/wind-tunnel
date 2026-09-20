# Étape 3 — Écrire les résultats, et les erreurs

**Fichiers :** `src/io/vtk.rs`, `src/error.rs` · **Vérification :** `cargo xtask goto 3` puis `cargo test` · **≈ 30 min**

Un calcul dont on ne voit rien ne sert à rien, et un calcul qui échoue sans le dire est
pire qu'inutile. Cette étape traite les deux faces de la même pièce : les sorties, et ce
qu'on raconte quand ça se passe mal.

## Socle

**`write_dataset`** — le format VTK legacy ASCII, volontairement daté : il tient en
cinquante lignes, se lit dans un éditeur de texte, et Paraview comme Tecplot l'ouvrent
sans discuter. La fonction écrit dans un `&mut impl Write`, pas dans un fichier : ouvrir
le fichier est le travail de `write_vtk`, qui l'appelle, et écrire dans un tampon mémoire
celui d'un test. Vous n'avez donc à produire que le contenu.

Le plus court chemin vers le format est un fichier complet. Voici un maillage de deux
cellules — un quadrangle et un triangle, portant un champ `c` — tel qu'il sort de
`write_header` puis `write_dataset` ; ParaView l'ouvre tel quel :

```text
            3             2                4
            +-------------+----------------+     cellule 0 : quadrangle 0 1 2 3
  y         |             |            /         cellule 1 : triangle   1 4 2
  ^         |     (0)     |   (1)  /             (sens direct dans les deux cas)
  |         |             |    /
  +--> x    +-------------+
            0             1
```

```text
# vtk DataFile Version 3.0
wind-tunnel                    ⎫
ASCII                          ⎬ les quatre lignes de `write_header`
DATASET UNSTRUCTURED_GRID      ⎭
POINTS 5 double
0 0 0                          ← sommet 0, toujours en 3D : x y z
1 0 0                          ← sommet 1
1 1 0
0 1 0
2 1 0                          ← sommet 4
CELLS 2 9                      ← 2 cellules, 9 entiers dans les lignes qui suivent
4 0 1 2 3                      ← nombre de sommets, puis leurs indices
3 1 4 2                        ← le triangle : 3 sommets
CELL_TYPES 2
9                              ← 9 = quadrangle
5                              ← 5 = triangle
CELL_DATA 2
SCALARS c double 1
LOOKUP_TABLE default
0.5                            ← c sur la cellule 0
0.25                           ← c sur la cellule 1
```

Les trois pièges du format sont tous visibles ici : les points sont toujours en 3D (d'où
le `0` final, ce calcul étant plan) ; le second nombre de la ligne `CELLS` est le total
des entiers des lignes qui suivent, nombres de sommets compris — ici
`(1 + 4) + (1 + 3) = 9` ; et les valeurs de `CELL_DATA` sont dans l'ordre des cellules, une
par cellule exactement. `cell.kind.vtk_code()` vous donne déjà `9` ou `5`. S'il y a
plusieurs champs, leurs blocs `SCALARS` / `LOOKUP_TABLE` se suivent sous le même
`CELL_DATA` — c'est le cas d'usage réel, `write_vtk` recevant une liste de champs.

La spécification complète du format legacy — les autres types de cellules, les sections
`POINT_DATA`, `VECTORS`, `FIELD` — est dans la documentation VTK :
<https://docs.vtk.org/en/latest/design_documents/VTKFileFormats.html>.

Ce que vous écrivez ici est le squelette du fichier : la géométrie, et un champ par
cellule. Les images d'un calcul complet en portent davantage — la vitesse, la fonction de
courant, la date — ajoutés par-dessus, hors de cette fonction. Le tableau de tous les
champs et leur lecture dans ParaView sont dans le `README.md`, section « Ce que contient
une sortie ».

## Ce qu'il y a à remarquer

**`?` est un `goto` discipliné.** Chaque `write!` peut échouer — disque plein, chemin
invalide. Le `?` renvoie immédiatement l'erreur à l'appelant si elle survient, et
continue sinon. Comparez au C, où l'oubli d'un test de retour est invisible, et où le
programme continue joyeusement à écrire dans un fichier qui n'existe plus.

**Les erreurs sont des types, pas des codes.** Regardez `src/error.rs` :

```rust
pub enum MeshError {
    Io(io::Error),
    RaggedMask { line: usize, expected: usize, got: usize },
    InvalidChar { line: usize, col: usize, ch: char },
    Disconnected { components: usize },
    ...
}
```

Chaque variante transporte de quoi diagnostiquer sans relire le code. `-1` ne dit pas où
est le caractère fautif ; `InvalidChar { line: 12, col: 47, ch: 'x' }`, si. Le compilateur
vérifie de surcroît qu'aucun appelant n'oublie de traiter le cas d'échec, puisqu'un
`Result` inutilisé déclenche un avertissement.

**`impl From<io::Error> for MeshError`** est ce qui permet au `?` de convertir tout seul
une erreur d'entrée-sortie en erreur de maillage. `thiserror` écrirait ces
implémentations à votre place ; il faut les avoir vues une fois.

**La chaîne des causes.** `Error::source` conserve l'erreur d'origine : `main` la déroule
et affiche « erreur : … / cause : … ». Rien n'est perdu en route.

## Pour aller plus loin

**`contains`**, dans `src/io/png.rs`, est aussi à trous : le test d'appartenance d'un
point à un polygone par lancer de rayon. Un rayon horizontal partant du point ; on compte
les arêtes traversées ; un nombre impair signifie « à l'intérieur ». Le rendu PNG en
dépend entièrement.

Puis essayez de casser le programme volontairement :

```shell
printf '...\n..\n'    > /tmp/ragged.dom && cargo run -- /tmp/ragged.dom
printf '...\n.x.\n'   > /tmp/etrange.dom && cargo run -- /tmp/etrange.dom
cargo run -- /tmp/inexistant.dom
```

Chacun doit produire un message qui suffit à corriger le fichier sans ouvrir le code.
