# Étape 11 — Passer à l'échelle : MPI et décomposition de domaine

**Fichiers :** `src/decomposition.rs`, `mpi/src/exchange.rs`, `mpi/src/main.rs`
· **Vérification :** `cargo xtask goto 11`, `cargo test`, puis `mpirun -n 4 …`
· **≈ 60 min** · *bonus*

**Prérequis rustlings :** `20_threads` (pour `Send`) et `18_iterators`. MPI lui-même n'a pas d'équivalent rustlings : voir les slides `rsmpi`.

Les étapes 9 et 10 ont fait travailler plusieurs cœurs sur **une seule** mémoire :
`par_iter()` et `thread::scope` supposent tous les deux que n'importe quel thread peut
lire n'importe quelle cellule. Au-delà d'une machine, il faut passer par le réseau,
avec pour chaque noeud, sa mémoire, sa part du domaine, et rien de commun sauf ce qu'ils
s'envoient : c'est MPI, et c'est ainsi que tournent les codes de calcul sur cluster. (une variante plus `std::net` est
possible mais moins commune dans le monde du calcul numérique)

Le point important n'est pas l'API de MPI — trois fonctions suffisent ici. C'est que **personne ne détient le domaine
complet**. Un rang qui maillerait toute la soufflerie
pour n'en calculer qu'un morceau n'aurait rien décomposé du tout : il aurait juste
gaspillé de la mémoire et gagné le droit de faire dix fois le même travail.

## L'idée

Le domaine est découpé en **bandes verticales**, une par rang. Chaque rang extrait sa
tranche de colonnes du masque, en appelle `Mesh::from_mask`, et fait tourner le solveur
dessus. Rien du solveur, du maillage, des schémas ni des sorties ne change : ils
travaillent déjà sur « un maillage », sans jamais supposer que c'est tout le domaine.

```text
masque global, 120 colonnes
+--------------------------------------------------+
|                     #####                        |
+--------------------------------------------------+
 <--- rang 0 ---><--- rang 1 ---><-- 2 --><--- 3 -->

ce que le rang 1 construit vraiment :
      +++----------------+++
      |||     #####      |||   hauteur complète, largeur 30 + 3 + 3
      +++----------------+++
       ^^^ fantômes       ^^^ fantômes
```

De chaque côté du domaine de calcul se trouvent des **cellules fantômes** : chaque rang les maille
et les calcule comme les autres, mais leurs valeurs associées ne lui appartiennent pas. Avant chaque
évaluation de résidu, il la redemande à son voisin — c'est l'*échange de halo*, la seule
communication du calcul.

Deux quantités doivent en plus être les mêmes partout :

- le **pas de temps**, sinon les rangs ne sont plus à la même date physique et le champ
  échangé aux coupures ne veut plus rien dire — une réduction `MIN` ;
- les **diagnostics** (masse, extrema), qui n'ont de sens que globaux — une réduction
  `SUM`, `MIN` et `MAX`. Attention : elles ne portent que sur les cellules *possédées*,
  sinon chaque cellule fantôme est comptée deux fois.

## Le piège à éviter

Envoyer avant de recevoir. Deux rangs voisins s'envoient mutuellement leur colonne : si
chacun poste d'abord un envoi **bloquant**, chacun attend que l'autre reçoive, et le
calcul s'arrête là. Sur des petits messages cela passe — l'implémentation les met dans un
tampon et rend la main — puis un jour le maillage grossit, le message dépasse le seuil de
bufferisation, et le code se fige sans rien dire. Postez les réceptions d'abord, avec des
primitives **immédiates** (`immediate_receive_into` *aka* `MPI_Irecv`, `immediate_send` *aka* `MPI_Isend`),
et attendez tout le monde d'un coup avec `wait_all` *aka* `MPI_Waitall`.

## Prérequis

Cette étape est la seule à demander une bibliothèque extérieure au projet :

```shell
brew install open-mpi        # macOS
apt install libopenmpi-dev   # Debian / Ubuntu
```

Le pilote MPI vit dans le crate `mpi/`, membre du workspace mais pas du groupe *par
défaut* : un workspace non virtuel ne construit que son paquet racine sans `-p`/
`--workspace`, donc `cargo build`, `cargo test` et `cargo clippy` à la racine ne le
construisent jamais, et le reste du projet continue de fonctionner sur une machine sans
MPI. Pour le bâtir :

```shell
cargo build --release -p wind-tunnel-mpi
```

## Socle

| Bloc             | Fichier                | Ce qu'il doit devenir                                                                                   |
|------------------|------------------------|---------------------------------------------------------------------------------------------------------|
| `Bands::owned`   | `src/decomposition.rs` | la répartition des colonnes en bandes aussi égales que possible, le reste sur les premières             |
| `mark_sends`     | `src/decomposition.rs` | ajouter aux envois les cellules possédées qui se trouvent dans le halo d'un voisin                     |
| `place`          | `src/decomposition.rs` | classer une cellule possédée ou fantôme, puis déléguer les envois                                       |
| `Layout::new`    | `src/decomposition.rs` | parcourir les cellules fluides dans l'ordre et déléguer leur classement à `place`                      |
| `pack_all`       | `mpi/src/exchange.rs`  | remplir les tampons d'envoi dans l'ordre des cellules du layout                                        |
| `transfer`       | `mpi/src/exchange.rs`  | réceptions immédiates puis envois dans `multiple_scope`, et `wait_all`                                 |
| `unpack_all`     | `mpi/src/exchange.rs`  | recopier les tampons reçus dans les cellules fantômes                                                   |
| `global_dt_max`  | `mpi/src/exchange.rs`  | `all_reduce_into` avec `SystemOperation::min()`                                                         |
| `time_loop`      | `mpi/src/main.rs`      | appeler la fermeture `advance` (qui enveloppe `advance_step`), puis déclencher le rapport périodique    |

Le corps d'`advance_step` est fourni : il expose les échanges avant chaque résidu et le
cas RK2 sans mélanger cette mécanique à la boucle à écrire. `Layout::empty` initialise
les listes de la bande ; `Layout::new` se concentre sur le parcours et la numérotation,
`place` sur la propriété et `mark_sends` sur les cellules du halo à envoyer.

Les quatre tampons d'un échange — deux à envoyer, deux à recevoir — vivent dans une
structure `Halo` créée une fois par rang, et non dans `exchange` : leur taille ne dépend
que du découpage, qui ne bouge plus une fois le maillage construit, alors qu'un échange a
lieu une à deux fois par pas de temps. C'est la même discipline que le tampon `work` de
`Solver::run`, et elle explique la forme de la signature : `Halo` traverse `time_loop` et
le rapporteur en `&mut` plutôt que d'être capturé par l'un d'eux, parce qu'il n'y en a
qu'un et que deux emprunts exclusifs simultanés ne passeraient pas.

Les deux premiers blocs ne parlent pas de MPI et se vérifient par `cargo test`, sans MPI
installé — c'est le plus gros de la difficulté, et c'est voulu : dans un code distribué,
ce qui casse est presque toujours « qui possède quoi », rarement le transport.

## Pourquoi trois colonnes de halo ?

La réponse se construit couche par couche, et la troisième est une surprise :

1. le résidu d'une cellule possédée lit la **valeur** de sa voisine — une couche ;
2. en MUSCL (étape 7), il lit aussi le **gradient** de cette voisine, calculé à partir
   des valeurs de la couche suivante — deux couches ;
3. ce gradient est un moindres carrés sur les **centroïdes** de la deuxième couche. Or le
   centroïde d'une cellule dépend de son chanfrein, et son chanfrein dépend de ses
   propres voisines (étape 2) — trois couches.

Autrement dit : *le maillage lui-même a un stencil*. Avec deux colonnes seulement, le
calcul à trois rangs sur `domains/tunnel.dom` s'écarte du calcul séquentiel de près de
`10⁻²` — pas un plantage, pas un interblocage, juste un résultat faux — alors que la
communication, elle, est parfaitement correcte. Essayez : passez `HALO` à 2 dans
`src/decomposition.rs` et relancez `cargo test`.

## Ce qu'il y a à remarquer

```shell
cargo test --test step11_decomposition
scripts/check-mpi.sh
```

`tests/step11_decomposition.rs` rejoue la décomposition **dans un seul processus**, en
échangeant les halos par recopie mémoire, et compare au calcul monolithique cellule par
cellule. Si ce test passe, l'algorithme est juste ; il ne reste au crate `mpi/` qu'à
transporter les mêmes tampons. `scripts/check-mpi.sh` fait ensuite tourner le vrai
binaire à 1, 2, 3 et 4 rangs et vérifie que les diagnostics coïncident avec ceux du
binaire séquentiel.

```shell
mpirun -n 4 target/release/wind-tunnel-mpi domains/tunnel.dom \
    --refine 4 --max-steps 200 --scheme muscl --time-scheme rk2 --out out-mpi
```

Chaque rang écrit sa bande : `rank2_frame_0010.vtk`. Il n'y a **aucun rassemblement** —
rapatrier le champ sur le rang 0 l'obligerait à mailler tout le domaine, exactement ce
qu'on vient d'éviter. Les fichiers portent des coordonnées globales, donc les bandes se
recollent au pixel près une fois affichées ensemble.

Le nom compte : `rank` avant `frame`, pas l'inverse. ParaView détecte une série
temporelle sur le seul nombre collé à l'extension — avec `frame` juste avant `.vtk`, il
ouvre `out-mpi/` comme une série par rang (`rank0_frame_*`, `rank1_frame_*`, …), chacune
animée sur le bon axe ; renversé (`frame_..._rankN.vtk`), il regroupe par rang et vous
retrouvez un « groupe » par pas de temps au lieu d'une série dans le temps.

## Pour aller plus loin

- Assemblez une image unique sur le rang 0. Les bandes étant verticales et de même
  hauteur, les sous-images se concatènent exactement : un `gather_varcount` des pixels,
  ou des valeurs, suffit. Comparez le coût de ce rassemblement à celui d'un pas de temps.
- Recouvrez calcul et communication : postez les échanges, calculez le résidu des
  cellules **intérieures** (celles dont aucune face ne touche une cellule fantôme)
  pendant que les messages circulent, et ne finissez les cellules du bord qu'après
  `wait_all`. C'est la technique standard pour que la communication devienne gratuite.
- Mesurez l'accélération : `--refine 13` donne de l'ordre du million de cellules.
  Chronométrez à 1, 2, 4 et 8 rangs. Le halo est proportionnel à la *hauteur* du domaine
  et le calcul à sa *surface* : plus les bandes sont fines, plus la part de communication
  grandit. À partir de combien de rangs cela ne vaut-il plus la peine ?
- `mpirun -n 4` sur une seule machine, ce sont quatre processus qui se parlent par
  mémoire partagée. Comparez avec `rayon` de l'étape 9 sur le même nombre de cœurs : que
  coûte le fait d'avoir renoncé à la mémoire commune ?

## Éléments de réponse

<details>
<summary>Assembler une image unique sur le rang 0</summary>

Chaque rang envoie les valeurs de ses cellules **possédées** (`layout.owned`), dans
l'ordre de sa bande ; le rang 0 les reçoit toutes en une fois :

```text
world.all_gather_into(&n_possédées, &mut counts)        // un entier par rang
displs = sommes préfixes de counts                      // le schéma CSR, encore
si rang 0 : buffer = vec![0.0; Σ counts]
            root.gather_varcount_into_root(&local,
                &mut PartitionMut::new(&mut buffer[..], counts, displs))
sinon     : root.gather_varcount_into(&local)
```

Le rang 0 doit alors connaître la place de chaque valeur dans le maillage complet : le
plus simple est qu'il construise lui-même le `Layout` de chaque bande (c'est déterministe,
`Layout::new` ne lit que le masque). Le rassemblement déplace tout le champ, `N` valeurs,
alors qu'un pas de temps n'échange que deux colonnes de halo : il ne se fait qu'aux pas
de sortie, jamais à chaque pas.

</details>

<details>
<summary>Recouvrir calcul et communication</summary>

```text
poster les réceptions et les envois immédiats (comme transfer, sans wait_all)
calculer le résidu des cellules intérieures   // aucune face vers une cellule fantôme
wait_all
calculer le résidu des cellules de bord de bande
```

La liste des cellules intérieures se calcule une fois, avec le `Layout`. Le résidu doit
alors pouvoir se calculer sur un sous-ensemble de cellules : c'est la seule modification du
solveur. Le gain est le temps de transfert du halo, gagné tant que le calcul intérieur
dure plus longtemps que lui — ce qui est vrai dès que les bandes sont larges.

</details>

<details>
<summary>À partir de combien de rangs cela ne vaut plus la peine</summary>

Sur une grille de `W × H` cellules coupée en `p` bandes verticales, chaque rang calcule
`W·H/p` cellules et échange `2·H` valeurs de halo par résidu. Le rapport communication
sur calcul vaut donc `2p / W` : il **croît avec `p`**, indépendamment de `H`. Quand les
bandes n'ont plus que quelques colonnes, le halo pèse autant que le calcul et
l'accélération plafonne. Un découpage en blocs (`√p × √p`) réduirait ce rapport à
`4√p / W` : c'est pourquoi les vrais codes ne découpent pas en bandes.

</details>

<details>
<summary><code>mpirun -n 4</code> contre <code>rayon</code> sur 4 cœurs</summary>

Sur une seule machine, `rayon` gagne en général : ses threads lisent directement le champ
voisin, alors que les processus MPI recopient les halos dans des tampons, les envoient
et les recopient à l'arrivée, et ajoutent une synchronisation à chaque échange. L'avantage
de MPI n'est pas là : il passe à **plusieurs machines**, ce que la mémoire partagée ne
peut pas faire.

</details>
