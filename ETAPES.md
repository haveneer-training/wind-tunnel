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

| # | Étape | Jour | ~Durée | Ce qu'on y voit de Rust |
|---|---|---|---|---|
| [0](docs/etapes/etape-00.md) | Géométrie de base | J1 | 20 min | types, fonctions, `Vec`, `derive`, tests |
| [1](docs/etapes/etape-01.md) | Le masque du domaine | J1 | 30 min | propriété, emprunts, slices |
| [2](docs/etapes/etape-02.md) | Maillage et connectivité | J1 | 40 min | `enum`, filtrage par motif, `HashMap`, `Option` |
| [3](docs/etapes/etape-03.md) | Écrire les résultats, et les erreurs | J1 | 30 min | `Result`, `?`, `Display`, `Error` |
| [4](docs/etapes/etape-04.md) | L'écoulement porteur | J2 | 25 min | traits, généricité |
| [5](docs/etapes/etape-05.md) | Le solveur | J2 | 45 min | itérateurs, erreurs typées, `&`/`&mut` |
| [6](docs/etapes/etape-06.md) | Qualité : conservation, ordre, `clippy` | J2 | 30 min | tests, documentation, outillage |
| [7](docs/etapes/etape-07.md) | Passer à l'ordre 2 en espace | J2/J3 | 40 min | généricité, `dyn`, mesure de performance |
| [8](docs/etapes/etape-08.md) | RK2 : pourquoi l'ordre n'avait pas bougé | J3 | 25 min | `enum` de schéma, relecture critique d'un résultat |
| [9](docs/etapes/etape-09.md) | Paralléliser avec `rayon` | J3 | 40 min | style fonctionnel, `par_iter`, performance |
| [10](docs/etapes/etape-10.md) | Threads : écriture recouverte, suivi | J3 | 30 min | `thread::scope`, `mpsc`, `Arc`, `Mutex` |
| 11 | *Bonus* : calculer l'écoulement | — | 40 min | algorithme itératif, convergence |
| 12 | *Bonus* : passage à l'échelle en MPI | — | — | décomposition de domaine |

Les étapes 0 à 5 forment le noyau : à la fin de l'étape 5, le code tourne et produit ses
premières images. Les suivantes l'améliorent.

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

Les étapes 9 et 10 abordent le parallélisme, sur un code dont la structure a été choisie
dès le départ pour s'y prêter. L'étape 9 traite le cas facile — chaque cellule écrit sa
propre case, `rayon` n'a besoin de rien d'autre — et l'étape 10 le cas où ça ne suffit
plus : plusieurs cellules dessinent dans la même image, il faut un verrou (`Mutex`), et
`thread::scope`/`mpsc` en sont l'occasion pour voir ce que `rayon` cache d'habitude.
