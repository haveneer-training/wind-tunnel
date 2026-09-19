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

```text
# vtk DataFile Version 3.0
wind-tunnel
ASCII
DATASET UNSTRUCTURED_GRID
POINTS <n> double
<x> <y> 0
...
CELLS <nb_cellules> <nb_entiers_total>
4 <v0> <v1> <v2> <v3>          ← chaque cellule précédée de son nombre de sommets
3 <v0> <v1> <v2>
...
CELL_TYPES <nb_cellules>
9                              ← 9 = quadrangle, 5 = triangle
CELL_DATA <nb_cellules>
SCALARS c double 1
LOOKUP_TABLE default
<valeur>
...
```

`cell.kind.vtk_code()` vous donne déjà le code de type.

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
