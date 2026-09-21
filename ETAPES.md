# Déroulé du fil rouge

Vous allez construire, étape par étape, un petit code de calcul complet : lecture d'un
domaine, génération d'un maillage non structuré, transport d'un traceur passif autour
d'un obstacle, écriture des résultats en VTK et en PNG. Ce que contient chaque étape, et
comment vous y travaillez, est expliqué ci-dessous.

## Le déroulement d'une étape

Le squelette est là, les tests aussi : ce qui manque, ce sont **30 `todo!()`**, répartis
sur treize étapes (0 à 12). Le corrigé complet est le dépôt d'où `travail/` a été
engendré (`cargo xtask start`) — consultez-le quand vous voulez, mais essayez d'abord.

### La boucle

Chaque étape tient en un énoncé court, quelques trous à combler, et des tests fournis
qui disent si c'est juste. Le principe est toujours le même :

```shell
cargo xtask goto 2   # ouvrir l'étape 2 (et compléter les précédentes restées vides)
cargo test           # rouge : voici ce qu'il faut écrire, fichier et ligne à l'appui
#   ... remplacer le todo!() entre les marqueurs >>> et <<< ...
cargo test           # vert  : l'étape est finie
```

`cargo test` ne montre que les étapes déjà ouvertes : jamais cinquante tests rouges d'un
coup, seulement ceux qui vous concernent. Chaque échec nomme le fichier et la ligne à
compléter.

Chaque énoncé comporte un **socle**, que tout le monde termine, et une **extension**,
facultative : personne n'attend son voisin. L'énoncé de chaque étape est dans
[`docs/etapes/`](docs/etapes/) — commencez toujours par le lire, il explique le
*pourquoi* autant que le *quoi*.

### Les commandes

| Commande | Effet |
|---|---|
| `cargo xtask status` | avancement étape par étape |
| `cargo xtask goto <n>` | passer à l'étape n ; remplit au passage les trous des étapes précédentes **restés vides** |
| `cargo xtask solve <n>` | remplir les trous de l'étape n à votre place (rattrapage) |
| `cargo xtask reset <n>` | rouvrir les trous de l'étape n pour la refaire |
| `cargo test` | vérifier |
| `cargo run --release -- domains/tunnel.dom` | faire tourner le calcul (à partir de l'étape 5) |

`goto` **n'écrase jamais** ce que vous avez écrit : il ne remplit que les blocs encore
occupés par un `todo!()`. Si vous décrochez sur une étape, `goto` la suivante et vous
repartez d'un code cohérent. Si vous voulez explicitement la réponse d'une étape,
`solve <n>` — et `reset <n>` si vous changez d'avis.

### Où écrire

Chaque trou est encadré ainsi :

```rust
pub fn dot(self, other: Vec2) -> f64 {
    // À FAIRE (étape 0) Produit scalaire de deux vecteurs du plan
    // >>> ÉTAPE 0 — à compléter
    todo!("étape 0 — voir le commentaire ci-dessus")
    // <<< ÉTAPE 0
}
```

Écrivez entre les deux marqueurs `>>>` et `<<<`, et laissez-les en place : c'est ainsi
que `goto`, `solve` et `reset` s'y retrouvent. Le reste du fichier vous appartient.

Un trou porte parfois la mention **« à écrire de zéro »** au lieu de « à compléter », et
n'a alors pas de `todo!()` :

```rust
// À FAIRE (étape 2) (conception, facultatif) Écrire ici les types de la connectivité…
// >>> ÉTAPE 2 — à écrire de zéro
// <<< ÉTAPE 2
```

C'est qu'il attend des **définitions** — des `struct`, des `enum`, des `impl` — et non le
corps d'une fonction qui existe déjà : `todo!()` est une expression, il ne pourrait pas y
tenir lieu de type. Il n'y en a qu'un, celui du bonus de conception
([`docs/etapes/etape-02-conception.md`](docs/etapes/etape-02-conception.md)), et il vit
dans le crate `design/`, à part. Tant qu'il est vide, `cargo test -p wind-tunnel-design`
ne **compile pas** — c'est normal, c'est l'exercice, et cela ne gêne ni `cargo test` ni
`cargo run`, qui ne touchent jamais ce crate.

Vous pouvez versionner votre travail — `git init && git add -A && git commit` — pour
retrouver vos états successifs.

### Les avertissements

Les **avertissements** du compilateur suivent la même règle que les tests. Un `todo!()`
rend mécaniquement inutilisés les paramètres de sa fonction ; ceux des étapes que vous
n'avez pas encore ouvertes sont tus, pour que vous ne lisiez que les vôtres. Ceux qui
restent disent quelque chose d'utile : « paramètre `points` inutilisé » sur la fonction
que vous êtes en train d'écrire, c'est la liste de ce qu'il vous reste à employer. Ils
disparaissent quand l'étape est finie.

## Les étapes

La colonne **« ce que vous écrivez »** est ce qui sort de votre clavier ; la colonne
**« ce que vous lisez »** est ce que l'étape donne à voir dans du code déjà écrit. Les
deux comptent, mais elles ne s'apprennent pas de la même façon — et le tableau dit
laquelle domine à chaque étape.

| # | Étape | Jour | ~Durée | Ce que vous écrivez | Ce que vous lisez |
|---|---|---|---|---|---|
| [0](docs/etapes/etape-00.md) | Géométrie de base | J1 | 20 min | types, fonctions, `Vec`, itérateurs | `derive`, doc-tests |
| [1](docs/etapes/etape-01.md) | Le masque du domaine | J1 | 35 min | indexation, `Vec`, boucles, `&self -> Mask` | propriété, déplacement, emprunts |
| [2](docs/etapes/etape-02.md) | Maillage et connectivité | J2 | 45 min | `match` exhaustif, `HashMap`, `enum` `Side`, CSR | indices typés, pointeurs vs indices |
| [3](docs/etapes/etape-03.md) | Écrire les résultats, et les erreurs | J2 | 25 min | `match` sur caractère, `Option`, `Err(...)`, `write!`/`?`, `From`, `Error::source` | `enum` d'erreurs, chaîne des causes |
| [4](docs/etapes/etape-04.md) | L'écoulement porteur | J2 | 20 min | `impl Trait for`, implémentation couvrante, `?Sized` | `Sync`, un trait comme besoin |
| [5](docs/etapes/etape-05.md) | Le solveur | J2 | 45 min | itérateurs, `Option`, `&`/`&mut` | *gather* vs *scatter*, CFL vérifiée avant |
| [6](docs/etapes/etape-06.md) | Qualité : conservation, ordre, `clippy` | J2 | 30 min | `fold` à accumulateur | tests de propriété, `clippy`, `cargo doc` |
| [7](docs/etapes/etape-07.md) | Passer à l'ordre 2 en espace | J2/J3 | 30 min | `match` sur `Option`, cas dégénéré | moindres carrés, limiteur, `dyn` vs générique |
| [8](docs/etapes/etape-08.md) | RK2 : pourquoi l'ordre n'avait pas bougé | J3 | 20 min | tampons, `clone`, `zip` | `enum` de schéma, relecture critique d'un résultat |
| [9](docs/etapes/etape-09.md) | Paralléliser avec `rayon` | J3 | 40 min | `par_iter`, fermetures, `Send`/`Sync` | ce que le compilateur refuse de paralléliser |
| [10](docs/etapes/etape-10.md) | Threads : écriture concurrente, suivi | J3 | 45 min | `thread::scope`, `Mutex`, `mpsc`, `chunks` | `Arc` et la propriété partagée |
| [11](docs/etapes/etape-11.md) | *Bonus* : passage à l'échelle en MPI | J3 | 60 min | découpage, possession, échanges immédiats | processus, messages, réductions |
| [12](docs/etapes/etape-12.md) | *Bonus* : calculer l'écoulement | J3 | 40 min | CSR aux sommets, `Option<f64>`, tampons échangés | algorithme itératif, résidu ≠ erreur |
| — | [*Bonus* : concevoir les types soi-même](docs/etapes/etape-02-conception.md) | J2 | 30 min | `struct`, `enum`, dérivations — **tout**, dans un fichier vide | le contrat d'un type, et ce que `mesh.rs` a choisi |
| — | [*Bonus* : chasse aux allocations](docs/BONUS-OPTIMISATION.md) | — | 45 min | tampons réutilisés, emprunts par champ | `GlobalAlloc`, mesure |

Les étapes 0 à 5 forment le noyau : à la fin de l'étape 5, le code tourne et produit ses
premières images. Les suivantes l'améliorent.

La colonne « Jour » suit l'ordre des notions, pas la commodité : les étapes 0 et 1 ne
demandent que ce que le J1 a vu avant elles — `Vec`, tranches, emprunts, boucles — et
**aucune** n'y exige d'`enum`, d'`Option` ni de `Result`. Les `struct` et les `enum` sont
travaillés en fin de J1, `Option` et `Result` ouvrent le J2 : les étapes 2 et 3 les
appliquent dans la foulée.

Le bonus « chasse aux allocations » ne s'ouvre pas avec `cargo xtask goto` : il est
transversal, se fait après les étapes 7 et 8, et reprend le code déjà écrit pour lui
retirer ce qu'il alloue dans sa boucle en temps.

Le bonus « concevoir les types soi-même » est le seul endroit du parcours où rien n'est
donné : un fichier vide, des tests, et à vous d'écrire les `struct` et les `enum`. Il
s'ouvre avec l'étape 2 (`cargo xtask goto 2`) mais vit dans un crate à part, qui ne gêne
rien tant qu'il n'est pas fini : `cargo test -p wind-tunnel-design`. Il se fait **avant**
de lire `src/mesh.rs`, faute de quoi il n'y a plus rien à concevoir — et donc au début du
J2, une fois les `enum` et `Option` présentés.

## Fil narratif

Les étapes 0 à 3 construisent la **donnée** : de la géométrie élémentaire jusqu'à un
maillage complet, validé et exportable. Rien ne bouge encore, mais tout ce qui suit en
dépend.

Les étapes 4 et 5 mettent le calcul en marche et donnent la première image.

Les étapes 6 à 8 forment une petite enquête. À l'étape 6, on mesure l'ordre de
convergence du schéma : environ 1, ce qui est normal pour un décentrement amont. À
l'étape 7, on passe la reconstruction spatiale à l'ordre 2 — et l'ordre mesuré **reste à
1**. L'étape 8 explique pourquoi (l'erreur en temps domine dès que `dt ∝ h`) et rétablit
l'ordre 2 avec une intégration Runge-Kutta. On y apprend autant sur la vérification d'un
code de calcul que sur le langage.

Les étapes 9 et 10 abordent le parallélisme **en mémoire partagée** (à distinguer du
passage à l'échelle en mémoire distribuée de l'étape 11, plus loin), sur un code dont la
structure a été choisie
dès le départ pour s'y prêter. L'étape 9 traite le cas facile — chaque cellule écrit sa
propre case, `rayon` n'a besoin de rien d'autre — et l'étape 10 le cas où ça ne suffit
plus : plusieurs cellules dessinent dans la même image, il faut un verrou (`Mutex`), et
`thread::scope`/`mpsc` en sont l'occasion pour voir ce que `rayon` cache d'habitude.

L'étape 11, en bonus, franchit la limite de la machine : plusieurs processus, chacun sa
mémoire, chacun sa bande du domaine. Le solveur n'y change pas d'une ligne — c'est le
signe qu'il ne supposait déjà plus que « son » maillage était tout le domaine — et la
difficulté se déplace là où elle est vraiment dans un code distribué : savoir qui possède
quoi.

L'étape 12, en bonus elle aussi, revient sur un défaut resté discret depuis l'étape 4 :
l'écoulement analytique ne voit ni la forme réelle de l'obstacle (réduite à un disque de
même aire), ni le maillage (une paroi en escalier n'est pas une de ses lignes de courant).
On calcule `ψ` aux sommets par `∇²ψ = 0`, résolu par balayages de Jacobi — et on vérifie
que la conservation, elle, ne dépendait jamais de la qualité de cet écoulement : elle vient
de la forme du schéma, pas de sa convergence.
