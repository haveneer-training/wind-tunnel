# Étape 1 — Le masque du domaine

**Fichier :** `src/mask.rs` · **Vérification :** `cargo xtask goto 1` puis `cargo test` · **≈ 35 min**

Le domaine de calcul est dessiné à la main dans un fichier texte : `.` pour du fluide,
`#` pour du solide. C'est confortable pour l'utilisateur, et c'est donc l'endroit exact
où le programme recevra des données fausses.

## Socle

| Fonction | Ce qu'elle doit faire |
|---|---|
| `Mask::is_fluid` | lire la bonne case de la grille, stockée à plat ligne par ligne dans un `Vec<bool>` ; `false` hors de la grille |
| `Mask::refine` | rendre un **nouveau** masque où chaque case est subdivisée en `factor × factor` |

Une case hors grille n'est pas une erreur : c'est l'extérieur de la veine, et il est
commode de pouvoir interroger les voisins d'une cellule de bord sans précaution
particulière.

`refine` ne redessine rien : le domaine reste le même, seule sa finesse augmente. Chacune
des `factor × factor` cases nouvelles reprend la valeur de celle dont elle vient — un
quotient entier suffit à la retrouver. C'est ce que fait l'option `--refine` en ligne de
commande, et ce dont l'étape 6 se servira pour mesurer un ordre de convergence : deux
maillages de finesses différentes sur le **même** domaine physique.

Une seule chose est à décider, et elle n'est pas dans la formule : `refine` reçoit `&self`
— elle *emprunte* le masque — et doit rendre un `Mask`. Elle ne peut donc pas modifier
celui qu'on lui donne ; il lui faut en construire un neuf et le rendre.

## Ce qu'il y a à remarquer

**Le masque est possédé, pas emprunté.** `Mask::parse` construit le `Vec<bool>`, puis le
*déplace* dans le `Mask` rendu à l'appelant — sans copie du tableau. Puis `Mesh::from_mask`
prend `&Mask` : il *emprunte* le masque le temps de construire le maillage, sans jamais le
modifier ni s'en approprier. Rien de tout cela n'est écrit à la main — c'est la signature
des fonctions qui le dit, et le compilateur qui le vérifie.

**Et c'est exactement ce que dit la signature de `refine`.** `&self` en entrée, `Mask` en
sortie : le masque d'origine survit à l'appel, intact, et le nouveau appartient à
l'appelant. `gros.refine(4)` ne consomme pas `gros`. En C, la même fonction rendrait un
pointeur, et il faudrait un commentaire — ou une convention de nommage — pour dire qui
doit le libérer, et si le masque d'origine a été touché au passage. Ici les deux réponses
sont dans le type, et le compilateur les fait respecter.

**`self.clone()` quand il n'y a rien à faire.** Raffiner par 1 ne change rien, mais on ne
peut pas pour autant « rendre `self` » : il n'est qu'emprunté. `clone()` en fabrique une
copie possédée, que l'on peut rendre. C'est le genre de détail que le compilateur vous
apprendra tout seul si vous essayez l'autre version.

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
