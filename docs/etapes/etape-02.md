# Étape 2 — Maillage et connectivité

**Fichiers :** `src/mask.rs`, `src/mesh.rs` · **Vérification :** `cargo xtask goto 2` puis `cargo test` · **≈ 45 min**

C'est le cœur de la structure de données. Une cellule ne connaîtra pas ses voisins par
arithmétique d'indices, mais par ses faces : tout ce qui sera écrit au-dessus
fonctionnerait donc à l'identique sur un maillage lu depuis un fichier.

> **Avant de lire ce fichier**, si vous en avez le temps :
> [étape 2, bonus — concevoir les types soi-même](etape-02-conception.md) vous fait écrire
> de zéro, dans un crate à part, les types que `mesh.rs` a déjà choisis. C'est le seul
> endroit du fil rouge où l'on conçoit au lieu de compléter, et il perd tout son sel une
> fois qu'on a lu la réponse.

## Socle

Le socle sépare quatre objectifs : la connexité du masque, la forme d'une cellule, la clé
canonique d'une arête, puis la création ou l'appariement d'une arête.

**Commencez par `Mask::check_connected`** (`src/mask.rs`) : `Mask::parse` l'appelle, donc
tant qu'elle vaut `todo!()`, aucun masque ne se lit et tous les tests de maillage
paniquent sur la même ligne. Jusqu'ici c'était un bouchon qui répondait toujours `Ok(())`.
L'exploration est divisée en deux fragments : `fluid_neighbours` donne les indices fluides
à quatre connexités, puis `explore` marque une composante depuis une cellule de départ.
`check_connected` lance ces explorations et compte les composantes ; un obstacle mal
dessiné qui coupe la veine en deux doit produire `MeshError::Disconnected { components }`
— votre premier `Err`. Le test `a_domain_split_in_two` de `tests/errors.rs` vous attend.

Ensuite, essayez :

```shell
printf '...\n###\n...\n' > /tmp/coupe.dom
cargo run --release -- /tmp/coupe.dom
```

et regardez ce que le programme répond. Comparez avec ce qu'un code C ferait d'un
domaine coupé en deux qu'il n'a pas vérifié.

**`cell_corners`** — les quatre coins d'une cellule, dans le sens direct, *sauf* quand
deux parois perpendiculaires se rejoignent : le coin qu'elles encadrent disparaît et la
cellule devient un triangle. Une marche d'escalier devient ainsi une facette à 45°.

Les quatre booléens `down`, `right`, `up`, `left` — eux aussi dans le sens direct —
disent si le voisin correspondant est une paroi. Quatre configurations donnent un
triangle, toutes les autres un quadrangle ; un `match` sur le quadruplet les couvre
exactement.

Attention à l'orientation : dans le sens direct, avec `y` vers le haut, l'ordre est
`bl → br → tr → tl`. Retirer un coin de cette liste conserve l'orientation.

**`build_faces`** parcourt les cellules et délègue chaque arête à `pair_edge`. La décision
locale est ainsi isolée et peut être testée sans écrire toute l'orchestration :

- première rencontre : on crée une face, provisoirement de bord — `boundary_face` vous
  est donnée, elle en calcule la normale, la longueur et le milieu ;
- deuxième rencontre : la face devient interne, `Side::Inner(cellule)` ;
- troisième rencontre : le maillage n'est pas une surface, `MeshError::NonManifoldEdge`.

**`edge_key`** fournit la paire de sommets **triée**, sans quoi les deux cellules
adjacentes ne tomberaient pas sur la même entrée du `HashMap`. Le helper isole une notion
de représentation importante — l'orientation ne change pas l'identité d'une arête — et
laisse `pair_edge` se concentrer sur ses trois cas de rencontre.

Le `match` imbriqué de cette fonction mérite d'être regardé pour lui-même : le premier
niveau distingue « arête déjà vue » de « arête nouvelle », le second, sur la face déjà
créée, distingue « elle attend encore son second voisin » de « elle en a déjà deux, donc
il y en a trois ». Le cas fautif n'est pas un `if` ajouté après coup : il tombe de
l'énumération des cas possibles.

## Ce qu'il y a à remarquer

**Des indices, pas des pointeurs.** La connectivité est faite de `CellId` et `FaceId`,
pas de `&Cell` ni de `*mut Cell`. Ce n'est pas un choix esthétique. En C, un tableau de
cellules agrandi par `realloc` peut être déplacé en mémoire : tous les `Cell*` distribués
ailleurs pointent alors dans le vide, et le programme continue comme si de rien n'était.
C'est l'un des bugs classiques des codes de maillage qui raffinent. Un indice, lui,
survit au déplacement du tableau. En Rust, la version à pointeurs ne compile même pas :
le vérificateur d'emprunts refuse qu'on modifie le `Vec` tant qu'une référence dedans est
vivante. La contrainte est la même dans les deux langages ; la différence est le moment
où on l'apprend.

**Trois types d'indices, pas un.** `CellId`, `FaceId` et `VertexId` sont trois types
distincts autour d'un `u32`. En C, ce seraient trois `int`, et passer un numéro de face
là où l'on attend une cellule compilerait sans un mot — pour donner un résultat faux,
plus tard, ailleurs. Ici, c'est une erreur de compilation. Le coût à l'exécution est nul.

**`Side` plutôt qu'`Option` + drapeau.** De l'autre côté d'une face, il y a soit une
cellule, soit une condition aux limites — jamais ni l'un ni l'autre, jamais les deux.
L'énumération rend cet état incohérent inexprimable, et le `match` du solveur ne peut
pas « oublier » le cas du bord : le compilateur exige qu'il soit traité.

**La connectivité inverse est provisoire.** `Mesh::build_cell_faces` (cellule → faces)
est fournie, dans une version simple : un `Vec` par cellule, mis bout à bout. L'étape 6
la réécrit au format CSR, sans aucun `Vec` intermédiaire.

## Pour aller plus loin

- `cargo test` vérifie une identité géométrique : la somme des normales
  orientées d'une cellule, pondérées par les longueurs, est nulle. Pourquoi est-ce vrai
  pour *tout* polygone fermé ?

  <details>
  <summary>Réponse</summary>

  Soit un polygone fermé de sommets `v_0, …, v_{n-1}` (et `v_n = v_0`). L'arête `i` porte
  le vecteur `e_i = v_{i+1} - v_i`. La normale sortante pondérée par la longueur de cette
  arête est `e_i` tourné de 90°.

  La somme des arêtes d'un polygone fermé est toujours nulle : `Σ e_i = Σ (v_{i+1} - v_i)`
  télescope, chaque sommet apparaissant une fois `+` et une fois `-`, jusqu'à revenir à
  `v_0`. Reste `v_n - v_0 = 0`.

  La rotation de 90° est une application linéaire, donc elle commute avec la somme :
  `Σ rot(e_i) = rot(Σ e_i) = rot(0) = 0`.

  Le résultat ne dépend ni de la forme du polygone, ni de sa convexité — seulement du fait
  qu'il est fermé. C'est la même identité qui, à l'étape 12, garantit que le flux net à
  travers une cellule fermée telescope à zéro.

  </details>
- Combien de triangles produit `domains/tunnel.dom` ? Changez le rayon du cylindre et
  observez.
