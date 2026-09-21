# Étape 2, bonus — Concevoir les types soi-même

**Fichier :** `design/src/lib.rs` · **Vérification :** `cargo test -p wind-tunnel-design`
· **≈ 30 min** · *facultatif, à faire au début de l'étape 2*

> Il vient juste après les `struct` et les `enum` du sas de fin de J1 : c'est leur
> première mise en œuvre sur un vrai sujet, et le seul endroit du fil rouge où la forme
> des types n'est pas donnée d'avance.

Partout ailleurs dans le fil rouge, les types sont donnés et vous en remplissez les
fonctions. Concevoir une structure de données — décider ce qui est un type, ce qui
est une valeur, ce qui ne doit pas pouvoir s'écrire — est ce qui distingue le plus
nettement Rust du C.

Cet exercice inverse donc la consigne : **le fichier est vide, et tout est à écrire.**

## Pourquoi un crate à part

Parce que la forme d'un type ou d'une interface est un contrat. La machinerie du 
fil-rouge impose que le code compile à chaque étape et l'absence d'un type casserait
la compilation vous donnant peu de matière pour vous guider. 

`design/` ne dépend de rien et rien ne dépend de lui. Il est membre du workspace mais pas
du groupe par défaut — exactement comme `mpi/` — si bien que `cargo test` à la racine ne
le construit jamais. Il peut donc rester cassé aussi longtemps qu'il vous plaira sans
gêner le fil rouge, et vous pouvez y écrire ce que vous voulez.

## Le sujet

On vous demande ici de concevoir un type `Side` qui représente les deux côtés d'une face. 
C'est une reproduction miniature de `src/mesh.rs` que vous retrouvez dans le projet principale.
Ce type doit respecter un contrat imposé par des tests (à la TDD).

Le domaine est fait de **cellules**, séparées par des **faces**. Une face est portée par
deux **sommets**. Elle a toujours une cellule d'un côté ; de l'autre, il y a soit une
seconde cellule, soit une frontière du domaine — une paroi, une entrée, une sortie.

Écrivez les types qui décrivent cela, avec le contrat suivant :

| Nom                       | Ce que c'est                                                                                                                                                                                   |
|---------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `CellId`, `VertexId`      | deux identifiants **distincts**, chacun autour d'un `u32`, comparables, copiables, affichables en mise au point, utilisables comme clé de `HashMap`, et sachant rendre leur `index() -> usize` |
| `BoundaryKind`            | la nature d'une frontière : `Wall`, `Inlet`, `Outlet`                                                                                                                                          |
| `Side`                    | ce qu'il y a de l'autre côté d'une face : une cellule, **ou** une frontière                                                                                                                    |
| `Face`                    | ses deux sommets `a` et `b`, sa cellule `left`, et son `right`                                                                                                                                 |
| `Face::is_boundary`       | la face est-elle sur une frontière ?                                                                                                                                                           |
| `Face::neighbour(from)`   | la cellule de l'autre côté, vue depuis `from` — et rien si `from` n'est pas une cellule de cette face, ou si la face est au bord                                                               |
| `neighbours(faces, cell)` | les voisines d'une cellule, dans l'ordre des faces                                                                                                                                             |

Les tests de `design/tests/connectivity.rs` sont donnés et fixent ces noms. Ils ne
compilent pas tant qu'il manque une définition : la première erreur les nomme toutes d'un
coup, et la liste se vide à mesure que vous écrivez. Lancez-les souvent.

## Trois décisions, et elles sont à vous

**`CellId` et `VertexId` doivent-ils être deux types ?** Rien ne vous oblige à les
séparer : `type CellId = u32` compilerait et les tests passeraient. Faites-le, puis
demandez-vous ce qui se passerait le jour où quelqu'un passe un numéro de face là où l'on
attend une cellule. En C, ce sont deux `int`, et la confusion compile sans un mot pour
donner un résultat faux plus tard, ailleurs. Ici, vous choisissez à quel moment vous
voulez l'apprendre. Le coût à l'exécution de la version sûre est **nul** : après
compilation, c'est un `u32`.

**`Side` doit-il être une énumération ?** L'autre forme possible est celle qu'on écrirait
en C : `right: Option<CellId>` plus un `kind: BoundaryKind` plus, peut-être, un booléen.
Elle compile, elle marche — et elle permet d'écrire des états qui ne veulent rien dire :
une face à la fois interne et au bord, ou ni l'un ni l'autre. Rien n'oblige à les tenir
cohérents, et le jour où ils divergent, le bug est loin de sa cause. L'énumération rend
l'état incohérent **inexprimable**, et le test `every_side_is_either_inner_or_boundary`
vous montrera l'autre moitié du marché : son `match` n'a pas de bras `_`, donc le
compilateur exigerait de traiter toute possibilité nouvelle.

Le test `the_enum_costs_no_more_than_a_flag` mesure la troisième moitié, celle qu'on
n'attend pas : l'énumération est aussi la plus **petite** des deux représentations.

**Que rend `neighbour` quand il n'y a pas de voisin ?** Une sentinelle — `u32::MAX`, ou
`-1` en C — et il faut penser à la tester à chaque appel. Un `Option<CellId>`, et le
compilateur ne vous laisse pas l'oublier. C'est le même réflexe que `max_stable_dt` à
l'étape 5.

## Puis comparez

Une fois vos tests verts, ouvrez `src/mesh.rs` et cherchez `CellId`, `Side`, `Face`. Le
projet a pris les mêmes décisions — c'est le sens de cet exercice — mais pas
nécessairement la même forme. Regardez en particulier :

- `Face` y porte davantage : normale, longueur, milieu, distance. Pourquoi ces
  quatre-là sont-ils rangés dans la face plutôt que recalculés à chaque usage ?
- la connectivité inverse (cellule → faces) n'y est pas un `Vec<Vec<FaceId>>` mais deux
  tableaux plats, le format CSR. C'est l'extension de l'étape 2.
- `Side::Boundary` y porte le même `BoundaryKind` que le vôtre, et le solveur en fait un
  `match` exhaustif : il ne *peut pas* oublier le cas du bord.

## Si vous décrochez

`cargo xtask solve 2` remplit ce fichier comme n'importe quel autre trou, et
`cargo xtask reset 2` le rouvre. Attention : les deux portent aussi sur les trous de
`src/mesh.rs`, cet exercice étant rattaché à l'étape 2. Et `cargo xtask goto 3` le
remplira tout seul en passant à la suite, comme toutes les extensions laissées de côté.
