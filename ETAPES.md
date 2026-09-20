# Déroulé du fil rouge

Chaque étape tient en un énoncé court, quelques trous à combler, et des tests fournis
qui disent si c'est juste. Le principe est toujours le même :

```shell
cargo xtask goto 2   # ouvrir l'étape 2 (et compléter les précédentes restées vides)
cargo test           # rouge : voici ce qu'il faut écrire, fichier et ligne à l'appui
#   ... remplacer le todo!() entre les marqueurs >>> et <<< ...
cargo test           # vert  : l'étape est finie
```

`cargo test` ne montre que les étapes déjà ouvertes : jamais cinquante tests rouges d'un
coup, seulement ceux qui vous concernent. `cargo xtask status` dit où vous en êtes,
`cargo xtask solve <n>` remplit une étape à votre place si vous décrochez, et
`cargo xtask reset <n>` la rouvre pour la refaire.

Chaque énoncé comporte un **socle**, que tout le monde termine, et une **extension**,
facultative : personne n'attend son voisin.

## Les étapes

La colonne **« ce que vous écrivez »** est ce qui sort de votre clavier ; la colonne
**« ce que vous lisez »** est ce que l'étape donne à voir dans du code déjà écrit. Les
deux comptent, mais elles ne s'apprennent pas de la même façon — et le tableau dit
laquelle domine à chaque étape.

| # | Étape | Jour | ~Durée | Ce que vous écrivez | Ce que vous lisez |
|---|---|---|---|---|---|
| [0](docs/etapes/etape-00.md) | Géométrie de base | J1 | 20 min | types, fonctions, `Vec`, itérateurs | `derive`, doc-tests |
| [1](docs/etapes/etape-01.md) | Le masque du domaine | J1 | 40 min | `match` sur caractère, `Option`, `&mut`, `Err(...)` | propriété, déplacement, emprunts |
| [2](docs/etapes/etape-02.md) | Maillage et connectivité | J1 | 45 min | `match` exhaustif, `HashMap`, `enum` `Side`, CSR | indices typés, pointeurs vs indices |
| [3](docs/etapes/etape-03.md) | Écrire les résultats, et les erreurs | J1 | 20 min | `write!`/`?`, `From`, `Error::source` | `enum` d'erreurs, chaîne des causes |
| [4](docs/etapes/etape-04.md) | L'écoulement porteur | J2 | 20 min | `impl Trait for`, implémentation couvrante, `?Sized` | `Sync`, un trait comme besoin |
| [5](docs/etapes/etape-05.md) | Le solveur | J2 | 45 min | itérateurs, `Option`, `&`/`&mut` | *gather* vs *scatter*, CFL vérifiée avant |
| [6](docs/etapes/etape-06.md) | Qualité : conservation, ordre, `clippy` | J2 | 30 min | `fold` à accumulateur | tests de propriété, `clippy`, `cargo doc` |
| [7](docs/etapes/etape-07.md) | Passer à l'ordre 2 en espace | J2/J3 | 30 min | `match` sur `Option`, cas dégénéré | moindres carrés, limiteur, `dyn` vs générique |
| [8](docs/etapes/etape-08.md) | RK2 : pourquoi l'ordre n'avait pas bougé | J3 | 20 min | tampons, `clone`, `zip` | `enum` de schéma, relecture critique d'un résultat |
| [9](docs/etapes/etape-09.md) | Paralléliser avec `rayon` | J3 | 40 min | `par_iter`, fermetures, `Send`/`Sync` | ce que le compilateur refuse de paralléliser |
| [10](docs/etapes/etape-10.md) | Threads : écriture recouverte, suivi | J3 | 45 min | `thread::scope`, `Mutex`, `mpsc`, `chunks` | `Arc` et la propriété partagée |
| [11](docs/etapes/etape-11.md) | *Bonus* : passage à l'échelle en MPI | J3 | 60 min | découpage, possession, échanges immédiats | processus, messages, réductions |
| [12](docs/etapes/etape-12.md) | *Bonus* : calculer l'écoulement | J3 | 40 min | CSR aux sommets, `Option<f64>`, tampons échangés | algorithme itératif, résidu ≠ erreur |
| — | [*Bonus* : chasse aux allocations](docs/BONUS-OPTIMISATION.md) | — | 45 min | tampons réutilisés, emprunts par champ | `GlobalAlloc`, mesure |

Les étapes 0 à 5 forment le noyau : à la fin de l'étape 5, le code tourne et produit ses
premières images. Les suivantes l'améliorent.

Le bonus « chasse aux allocations » ne s'ouvre pas avec `cargo xtask goto` : il est
transversal, se fait après les étapes 7 et 8, et reprend le code déjà écrit pour lui
retirer ce qu'il alloue dans sa boucle en temps.

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
