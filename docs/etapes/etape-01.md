# Étape 1 — Le masque du domaine

**Fichier :** `src/mask.rs` · **Vérification :** `cargo test mask` puis `cargo test --test errors` · **≈ 30 min**

Le domaine de calcul est dessiné à la main dans un fichier texte : `.` pour du fluide,
`#` pour du solide. C'est confortable pour l'utilisateur, et c'est donc l'endroit exact
où le programme recevra des données fausses.

## Socle

Un seul trou : **`Mask::is_fluid`**. La grille est stockée à plat, ligne par ligne, dans
un `Vec<bool>` ; il faut répondre `false` hors de la grille et lire la bonne case sinon.

Une case hors grille n'est pas une erreur : c'est l'extérieur de la veine, et il est
commode de pouvoir interroger les voisins d'une cellule de bord sans précaution
particulière.

## Ce qu'il y a à remarquer

**Le masque est possédé, pas emprunté.** `Mask::parse` construit un `Mask` et le rend :
la valeur est *déplacée* vers l'appelant, sans copie du tableau. Puis `Mesh::from_mask`
prend `&Mask` : il *emprunte* le masque le temps de construire le maillage, sans jamais
le modifier ni s'en approprier. Rien de tout cela n'est écrit à la main — c'est la
signature des fonctions qui le dit, et le compilateur qui le vérifie.

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

Le test `domaine_coupe_en_deux` de `tests/erreurs.rs` vous attend.

Ensuite, essayez :

```shell
printf '...\n###\n...\n' > /tmp/coupe.dom
cargo run --release -- /tmp/coupe.dom
```

et regardez ce que le programme répond. Comparez avec ce qu'un code C ferait d'un
domaine coupé en deux qu'il n'a pas vérifié.
