# Étape 1 — Le masque du domaine

**Fichier :** `src/mask.rs` · **Vérification :** `cargo xtask goto 1` puis `cargo test` · **≈ 40 min**

Le domaine de calcul est dessiné à la main dans un fichier texte : `.` pour du fluide,
`#` pour du solide. C'est confortable pour l'utilisateur, et c'est donc l'endroit exact
où le programme recevra des données fausses.

## Socle

| Fonction | Ce qu'elle doit faire |
|---|---|
| `Mask::is_fluid` | lire la bonne case de la grille, stockée à plat ligne par ligne dans un `Vec<bool>` ; `false` hors de la grille |
| `parse_row` | analyser une ligne du fichier : largeur attendue, caractères admis, cellules empilées |

Une case hors grille n'est pas une erreur : c'est l'extérieur de la veine, et il est
commode de pouvoir interroger les voisins d'une cellule de bord sans précaution
particulière.

`parse_row` est l'endroit où le format est validé, et il a trois choses à dire :

- la ligne n'a pas la largeur attendue → `MeshError::RaggedMask { line, expected, got }` ;
- un caractère n'est ni `.` ni `#` → `MeshError::InvalidChar { line, col, ch }` ;
- tout va bien → la largeur de la ligne, que l'appelant retient comme référence pour les
  suivantes.

D'où le paramètre `expected: Option<usize>` : `None` sur la première ligne — c'est elle
qui fixe la largeur — et `Some(largeur)` ensuite. L'`Option` dit dans le type qu'il n'y a
pas toujours de référence à comparer, plutôt que de faire passer un `0` pour « pas encore
de largeur ».

Les numéros de ligne et de colonne sont comptés **à partir de 1** : ce sont des numéros
destinés à un humain qui ouvrira le fichier dans un éditeur, pas des indices.

## Ce qu'il y a à remarquer

**Le masque est possédé, pas emprunté.** `Mask::parse` construit le `Vec<bool>`, le prête
en `&mut` à `parse_row` qui le remplit ligne après ligne, puis le *déplace* dans le `Mask`
rendu à l'appelant — sans copie du tableau. Puis `Mesh::from_mask` prend `&Mask` : il
*emprunte* le masque le temps de construire le maillage, sans jamais le modifier ni s'en
approprier. Rien de tout cela n'est écrit à la main — c'est la signature des fonctions qui
le dit, et le compilateur qui le vérifie.

**Un `&mut Vec<bool>` est un prêt exclusif**, et c'est ce qui rend `parse_row` sûre : tant
qu'elle le tient, personne d'autre ne peut lire ni écrire le tableau. En C, la même
fonction recevrait un `bool*` et une capacité, et rien ne garantirait qu'un autre bout du
programme ne s'en sert pas au même moment.

**Le rappel qui vaut pour la suite.** En C, la question « qui libère ce tableau, et
quand ? » se règle par convention, par commentaire, ou par accident. Ici elle a une
réponse dans le type : le `Mask` est libéré quand la variable qui le possède sort de sa
portée, et il est impossible d'en conserver un pointeur au-delà. Nous verrons à l'étape
suivante ce que cela change concrètement pour une structure de maillage.

**`self.fluid[k]` est vérifié à l'exécution.** Un indice hors bornes provoque un `panic`
immédiat et localisé, pas une lecture silencieuse de la mémoire du voisin. Le coût de ce
contrôle est presque toujours nul, car le compilateur l'élimine quand il peut prouver
que l'indice est valide — et nous mesurerons cela à l'étape 9.

## Pour aller plus loin

**`Mask::check_connected`** est également à trous. Un obstacle mal dessiné peut couper la
veine en deux morceaux ; le calcul n'aurait alors aucun sens. Comptez les composantes
connexes du fluide par un parcours en largeur, et renvoyez
`MeshError::Disconnected { components }` s'il y en a plus d'une.

Le test `a_domain_split_in_two` de `tests/errors.rs` vous attend.

Ensuite, essayez :

```shell
printf '...\n###\n...\n' > /tmp/coupe.dom
cargo run --release -- /tmp/coupe.dom
```

et regardez ce que le programme répond. Comparez avec ce qu'un code C ferait d'un
domaine coupé en deux qu'il n'a pas vérifié.
