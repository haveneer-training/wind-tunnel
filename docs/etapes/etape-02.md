# Étape 2 — Maillage et connectivité

**Fichier :** `src/mesh.rs` · **Vérification :** `cargo xtask goto 2` puis `cargo test` · **≈ 45 min**

C'est le cœur de la structure de données. Une cellule ne connaîtra pas ses voisins par
arithmétique d'indices, mais par ses faces : tout ce qui sera écrit au-dessus
fonctionnerait donc à l'identique sur un maillage lu depuis un fichier.

## Socle

Deux trous : la forme d'une cellule, puis l'appariement de leurs arêtes.

**`cell_corners`** — les quatre coins d'une cellule, dans le sens direct, *sauf* quand
deux parois perpendiculaires se rejoignent : le coin qu'elles encadrent disparaît et la
cellule devient un triangle. Une marche d'escalier devient ainsi une facette à 45°.

Les quatre booléens `down`, `right`, `up`, `left` — eux aussi dans le sens direct —
disent si le voisin correspondant est une paroi. Quatre configurations donnent un
triangle, toutes les autres un quadrangle ; un `match` sur le quadruplet les couvre
exactement.

Attention à l'orientation : dans le sens direct, avec `y` vers le haut, l'ordre est
`bl → br → tr → tl`. Retirer un coin de cette liste conserve l'orientation.

**`build_faces`** — l'appariement des arêtes. On parcourt toutes les cellules, et pour
chaque arête `(a, b)` :

- première rencontre : on crée une face, provisoirement de bord — `boundary_face` vous
  est donnée, elle en calcule la normale, la longueur et le milieu ;
- deuxième rencontre : la face devient interne, `Side::Inner(cellule)` ;
- troisième rencontre : le maillage n'est pas une surface, `MeshError::NonManifoldEdge`.

La clé est la paire de sommets **triée**, sans quoi les deux cellules adjacentes ne
tomberaient pas sur la même entrée du `HashMap`.

Le `match` imbriqué de cette fonction mérite d'être regardé pour lui-même : le premier
niveau distingue « arête déjà vue » de « arête nouvelle », le second, sur la face déjà
créée, distingue « elle attend encore son second voisin » de « elle en a déjà deux, donc
il y en a trois ». Le cas fautif n'est pas un `if` ajouté après coup : il tombe de
l'énumération des cas possibles.

## Extension

**`Mesh::build_cell_faces`** — la connectivité inverse, cellule → faces, au format CSR
décrit plus bas. Trois temps, et c'est un schéma qui revient partout en calcul :

1. compter les faces de chaque cellule (une face interne compte pour ses **deux**
   cellules) ;
2. cumuler ces compteurs en décalages — `offsets[i]` dit où commencent les faces de la
   cellule `i` ;
3. remplir, en avançant un curseur par cellule.

L'étape 12, si vous y allez, reprend exactement ce schéma pour le voisinage des sommets.

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

**Le stockage CSR.** `cell_face_offsets` et `cell_face_indices` encodent une liste de
longueur variable par cellule dans deux tableaux plats, sans un `Vec` par cellule. C'est
la structure qu'on retrouve partout en calcul : contiguë, sans indirection, aimable avec
le cache.

## Pour aller plus loin

- `cargo test` vérifie une identité géométrique : la somme des normales
  orientées d'une cellule, pondérées par les longueurs, est nulle. Pourquoi est-ce vrai
  pour *tout* polygone fermé ?
- Combien de triangles produit `domains/tunnel.dom` ? Changez le rayon du cylindre et
  observez.
