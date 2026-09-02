# Étape 11 — Passer à l'échelle : MPI et décomposition de domaine

**Fichiers :** `src/decomposition.rs`, `mpi/src/exchange.rs`, `mpi/src/main.rs`
· **Vérification :** `cargo xtask goto 11`, `cargo test`, puis `mpirun -n 4 …`
· **≈ 60 min** · *bonus*

Les étapes 9 et 10 ont fait travailler plusieurs cœurs sur **une seule** mémoire :
`par_iter()` et `thread::scope` supposent tous les deux que n'importe quel thread peut
lire n'importe quelle cellule. Au-delà d'une machine, cette hypothèse tombe. Plusieurs
processus, chacun sa mémoire, chacun sa part du domaine, et rien de commun sauf ce qu'ils
s'envoient : c'est MPI, et c'est ainsi que tournent les codes de calcul sur cluster.

Le point important n'est pas l'API de MPI — trois fonctions suffisent ici. C'est que
**personne ne détient le domaine complet**. Un rang qui maillerait toute la soufflerie
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

Les colonnes en trop de chaque côté sont les **cellules fantômes** : le rang les maille
et les calcule comme les autres, mais leur valeur ne lui appartient pas. Avant chaque
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
primitives **immédiates** (`immediate_receive_into`, `immediate_send`), et attendez tout
le monde d'un coup avec `wait_all`.

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

| Bloc | Fichier | Ce qu'il doit devenir |
|---|---|---|
| `Bands::owned` | `src/decomposition.rs` | la répartition des colonnes en bandes aussi égales que possible, le reste sur les premières |
| `Layout::new` | `src/decomposition.rs` | le tri des cellules de la bande : possédées, à envoyer à gauche/droite, à recevoir de gauche/droite |
| `Halo::exchange` | `mpi/src/exchange.rs` | `pack_into`, réceptions immédiates puis envois immédiats dans un `multiple_scope`, `wait_all`, `unpack` |
| `global_dt_max` | `mpi/src/exchange.rs` | `all_reduce_into` avec `SystemOperation::min()` |
| `time_loop` | `mpi/src/main.rs` | la boucle en temps du pilote : un `halo.exchange` avant **chaque** `residual` |

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
cargo test --test decomposition
scripts/check-mpi.sh
```

`tests/decomposition.rs` rejoue la décomposition **dans un seul processus**, en
échangeant les halos par recopie mémoire, et compare au calcul monolithique cellule par
cellule. Si ce test passe, l'algorithme est juste ; il ne reste au crate `mpi/` qu'à
transporter les mêmes tampons. `scripts/check-mpi.sh` fait ensuite tourner le vrai
binaire à 1, 2, 3 et 4 rangs et vérifie que les diagnostics coïncident avec ceux du
binaire séquentiel.

```shell
mpirun -n 4 target/release/wind-tunnel-mpi domains/tunnel.dom \
    --refine 4 --steps 200 --scheme muscl --time-scheme rk2 --out out-mpi
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
