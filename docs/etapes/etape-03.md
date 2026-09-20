# Étape 3 — Écrire les résultats, et les erreurs

**Fichiers :** `src/mask.rs`, `src/io/vtk.rs`, `src/error.rs` · **Vérification :** `cargo xtask goto 3` puis `cargo test` · **≈ 25 min**

Un calcul dont on ne voit rien ne sert à rien, et un calcul qui échoue sans le dire est
pire qu'inutile. Cette étape traite les deux faces de la même pièce : les sorties, et ce
qu'on raconte quand ça se passe mal — à l'entrée du programme comme à sa sortie.

Elle vient donc juste après les `enum`, `Option`, `Result` et l'opérateur `?` : c'est leur
première application sur de vraies erreurs métier. Vous reviendrez pour cela dans
`mask.rs`, dont la lecture était restée fonctionnelle mais muette jusqu'ici.

Un test rouge de l'étape 1 qui reparaît à l'ouverture de cette étape n'est donc pas une
régression : `Mask::parse` passe par la fonction que vous allez écrire.

## Socle

| Fonction | Fichier | Ce qu'elle doit faire |
|---|---|---|
| `parse_row` | `mask.rs` | valider une ligne du masque, et **nommer** ce qui cloche |
| `write_cells` | `io/vtk.rs` | les sections `CELLS` et `CELL_TYPES` du fichier |
| `MeshError::source` | `error.rs` | l'erreur d'origine, quand il y en a une sous celle-ci |
| `From<io::Error> for MeshError` | `error.rs` | la conversion qui fait marcher le `?` |

### Les erreurs de lecture du masque

`parse_row` est l'endroit où le format dessiné à la main est validé — donc l'endroit
exact où le programme reçoit des données fausses. Elle a trois choses à dire :

- la ligne n'a pas la largeur attendue → `MeshError::RaggedMask { line, expected, got }` ;
- un caractère n'est ni `.` ni `#` → `MeshError::InvalidChar { line, col, ch }` ;
- tout va bien → la largeur de la ligne, que `Mask::parse` retient comme référence pour
  les suivantes.

D'où le paramètre `expected: Option<usize>` : `None` sur la première ligne — c'est elle
qui fixe la largeur — et `Some(largeur)` ensuite. L'`Option` dit **dans le type** qu'il
n'y a pas toujours de référence à comparer, plutôt que de faire passer un `0` pour « pas
encore de largeur ». C'est la même idée que les variantes ci-dessous : l'information est
portée par le type, pas par une valeur convenue.

Les numéros de ligne et de colonne sont comptés **à partir de 1** : ils sont destinés à un
humain qui ouvrira le fichier dans un éditeur, pas à indexer un tableau.

Le `Vec<bool>` arrive en `&mut` : un prêt **exclusif**. Tant que `parse_row` le tient,
personne d'autre ne peut ni le lire ni l'écrire. En C, la fonction recevrait un `bool*` et
une capacité, et rien ne garantirait le contraire.

### Les sorties

Le format VTK legacy ASCII est volontairement daté : il tient en cinquante lignes, se lit
dans un éditeur de texte, et Paraview comme Tecplot l'ouvrent sans discuter.
`write_dataset` l'écrit en trois temps — `write_points`, `write_cells`,
`write_cell_data` — dont le premier et le dernier vous sont **donnés** : ils vous servent
de modèle, il ne reste que la connectivité, où sont les deux pièges du format.

Ces fonctions écrivent dans un `&mut impl Write`, pas dans un fichier : ouvrir le fichier
est le travail de `write_vtk`, qui les appelle, et écrire dans un tampon mémoire celui
d'un test. Vous n'avez donc à produire que le contenu.

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

Les trois pièges du format sont tous visibles ici, et **les deux qui vous concernent sont
dans les sections que vous écrivez** : le second nombre de la ligne `CELLS` est le total
des entiers des lignes qui suivent, nombres de sommets compris — ici
`(1 + 4) + (1 + 3) = 9` — et chaque cellule s'annonce par son nombre de sommets avant de
les lister. `cell.kind.vtk_code()` vous donne déjà le `9` ou le `5` de `CELL_TYPES`.

Le troisième est dans `write_points`, qui vous est donnée : les points sont toujours en
3D, d'où le `0` final, ce calcul étant plan. Et dans `write_cell_data`, également donnée :
les valeurs sont dans l'ordre des cellules, une par cellule exactement, et s'il y a
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
une erreur d'entrée-sortie en erreur de maillage — c'est le deuxième trou de l'étape, et
il tient en une ligne. Retirez-le, et `fs::read_to_string(path)?` dans `Mask::from_file`
ne compile plus : le `?` cherche un `From<io::Error>` et ne le trouve pas. `thiserror`
écrirait cette implémentation à votre place ; il faut l'avoir vue une fois.

**La chaîne des causes.** `Error::source` — le troisième trou — conserve l'erreur
d'origine : `main` la déroule et affiche « erreur : … / cause : … ». Le message de haut
niveau dit ce que le programme essayait de faire, la cause dit ce que le système a
répondu, et rien n'est recopié de l'un dans l'autre. Le test `a_missing_file` de
`tests/errors.rs` vérifie les deux bouts de la chaîne.

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
