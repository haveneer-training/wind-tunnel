# Étape 5 — Le solveur

**Fichiers :** `src/flux.rs`, `src/solver.rs` · **Vérification :** `cargo xtask goto 5`, `cargo test`, puis `cargo run --release -- domains/tunnel.dom` · **≈ 45 min**

Tout est en place : un maillage, un écoulement, des sorties. Il reste à faire avancer le
traceur — et, à la fin de cette étape, à regarder l'image.

## Le schéma

Sur chaque cellule, on écrit le bilan de ce qui entre et sort :

```text
|Ωi| · dcᵢ/dt = − Σ_faces [ débit · c_face − D (c_voisin − cᵢ) L / d ]
```

`c_face` est la valeur transportée à travers la face. Le décentrement amont la prend du
côté d'où vient le fluide : simple, jamais oscillant, et diffusif — un front net s'étale
en traversant le domaine. C'est précisément le défaut que l'étape 7 corrigera.

La stabilité impose un pas de temps maximal :

```text
dt ≤ min_i  |Ωi| / Σ_faces ( |débit| + 2·D·L/d )
```

## Socle

| Fonction | Fichier | Ce qu'elle doit faire |
|---|---|---|
| `Upwind::interface_value` | `flux.rs` | retenir la valeur du côté d'où vient le fluide |
| `max_stable_dt` | `solver.rs` | la formule CFL ci-dessus, ou `None` si aucune cellule n'a de somme strictement positive |
| `Solver::step` | `solver.rs` | calculer le résidu, puis avancer de `dt · résidu` |

`Solver::residual`, plus longue, vous est donnée : lisez-la, c'est elle qui met tout le
reste en musique.

## Ce qu'il y a à remarquer

**La boucle est écrite en *gather*.** On boucle sur les cellules, et chaque cellule lit
ses faces pour accumuler *son* résidu. Personne n'écrit chez le voisin. La formulation
duale, en *scatter* — boucler sur les faces et ajouter le flux aux deux cellules
adjacentes — est plus naturelle à écrire et fait moitié moins de calculs. Elle devient
aussi une course de données dès qu'on la parallélise : deux threads traitant deux faces
d'une même cellule s'écrasent mutuellement. En C ou en Fortran avec OpenMP, cette
parallélisation se fait en une ligne, tourne, et donne un résultat faux de façon non
reproductible. En Rust, elle ne compile pas. Nous y reviendrons à l'étape 9, ce qui
expliquera pourquoi la structure a été choisie ainsi dès le départ.

**`residual(&self, c: &Field, out: &mut Field)`.** Un champ en lecture, un en écriture
exclusive. Il est *impossible* de passer deux fois le même champ : le compilateur le
refuse. En C, le même appel avec le même pointeur des deux côtés compile, tourne, et
donne un résultat silencieusement faux ; `restrict` ne fait que promettre le contraire, et
la promesse n'est pas vérifiée.

**La boucle en temps n'alloue rien.** Le tampon `work` est alloué une fois avant la
boucle, puis réutilisé. Une allocation par pas de temps ne se voit pas sur un petit cas,
et coûte cher sur un gros.

**`Option` plutôt qu'une valeur sentinelle.** `max_stable_dt` renvoie
`Option<f64>` : le minimum porte sur les cellules dont la somme des débits est
strictement positive, et si aucune ne l'est — écoulement nul *et* diffusivité nulle —
il n'y a pas de minimum du tout. En C on renverrait `DBL_MAX` ou `-1`, un appelant
oublierait de le tester, et `0,4 × DBL_MAX` donnerait un pas de temps de 10³⁰⁸ secondes
sans que rien ne proteste. Ici le type dit qu'il peut ne pas y avoir de réponse, et
`Solver::new` est obligé d'en faire quelque chose — en l'occurrence
`SolverError::NoTransport`. C'est le même réflexe que `Side` plutôt qu'un
`Option<CellId>` doublé d'un drapeau, à l'étape 2 : rendre l'état douteux
inexprimable.

**Le CFL est vérifié avant de calculer**, pas après. Une intégration explicite instable ne
donne pas un résultat approximatif : elle donne du bruit, puis des `NaN`. Autant le dire
tout de suite, avec les deux valeurs en cause :

```shell
cargo run --release -- domains/tunnel.dom --dt 42
```

**Et pendant le calcul, on surveille.** `check_every` déclenche un contrôle de finitude
périodique ; à la première valeur non finie, le calcul s'arrête en nommant le pas et la
cellule fautive. Un `NaN` qui se propage en silence jusqu'à un résultat publié est une
histoire vraie, dans à peu près tous les laboratoires.

## Le résultat

```shell
cargo run --release -- domains/tunnel.dom --steps 400 --every 20 --circulation 4
```

Les filets de fumée se déforment autour du cylindre, dissymétriquement à cause de la
circulation. Le champ reste rigoureusement dans `[0, 1]` — vérifiez-le dans les lignes de
sortie —, et la masse décroît régulièrement à mesure que la fumée sort par l'aval.

## Pour aller plus loin

- Ajoutez `--diffusivity 0.05` : le pas de temps stable chute. Retrouvez pourquoi dans la
  formule CFL.
- `Centered`, dans `flux.rs`, est déjà implémenté : un schéma d'ordre 2 en espace.
  Essayez `--scheme centered` et lisez les bornes affichées à chaque sortie. Elles
  passent de `[0, 1]` à `[-0,49 ; 1,37]` après 50 pas, `[-2,46 ; 2,28]` après 150 :
  des concentrations négatives, dont l'amplitude croît. Poussez à `--steps 4000` et le
  champ atteint 10¹⁶. Le décentrement amont, lui, reste rigoureusement borné. « Ordre
  plus élevé » et « meilleur » sont deux choses différentes, et c'est exactement le
  problème que devra résoudre l'étape 7.
- Raffinez avec `--refine 4` : chaque case du masque est subdivisée en 4×4, soit
  90 000 cellules au lieu de 5 600, sur le **même** domaine physique. L'interface entre
  bandes s'affine et le pas de temps stable est divisé par quatre. Chronométrez : c'est
  le sujet des étapes 9 et 10.
- Attention à ne pas confondre avec `--h`, qui fixe la *taille* d'une cellule et non
  leur nombre : le diviser par deux rétrécit le domaine de moitié et redonne exactement
  la même image. Seul le masque décide de la résolution.
- `--bands 9` densifie le rideau de fumée. Avec un nombre pair, une bande passe
  frontalement sur l'obstacle au lieu de l'encadrer.

### D'où vient exactement l'étalement ?

La diffusivité vaut zéro par défaut : dans la solution exacte, un champ valant 0 ou 1
transporté par un écoulement à divergence nulle **reste** exactement 0 ou 1, seule la
forme de l'interface change. Toute valeur intermédiaire à l'écran est donc de l'erreur
numérique, et rien d'autre.

Le masque `domains/tunnel-empty.dom` — une veine vide, sans obstacle — permet de
l'isoler, parce qu'il rend l'inclinaison de l'écoulement réglable par `--angle` :

```shell
cargo run --release -- domains/tunnel-empty.dom --refine 4 --bands 9 --angle 0
cargo run --release -- domains/tunnel-empty.dom --refine 4 --bands 9 --angle 45
```

À 0°, l'écoulement suit les axes du maillage et longe les bandes : le schéma est
**exact**, les interfaces restent parfaitement nettes d'un bout à l'autre du domaine.
Après 480 pas, seules 3 % des cellules portent une valeur intermédiaire — et ce sont
celles du front d'air pur qui entre par l'amont, la seule interface perpendiculaire à
l'écoulement.

À 45°, sans le moindre obstacle, 70 % des cellules sont floutées et les bandes ont
fondu. C'est la « fausse diffusion » des schémas d'ordre 1, d'amplitude proportionnelle
à `|u|·Δx·sin 2θ` : elle est nulle quand l'écoulement est aligné avec la grille, et
maximale à 45°.

D'où la conclusion qui justifie l'étape 7 : sur un maillage non structuré, l'écoulement
n'est *jamais* aligné avec les faces. Un schéma d'ordre 1 y est structurellement
condamné à cette erreur. Et sur le cas avec obstacle, ce n'est pas l'obstacle qui
diffuse — c'est lui qui rend l'écoulement oblique, ce qui déclenche la fausse diffusion.

Deux détails que ces essais révèlent, et qui valent d'être regardés :

- à 45°, la masse **augmente** de 10 % : la paroi basse est devenue une frontière
  d'entrée, et une condition de gradient nul y réinjecte du traceur. Un gradient nul sur
  une frontière d'entrée est un problème mal posé — le code ne s'en plaint pas, c'est à
  vous de le voir ;
- `--angle` est refusé sur un masque avec obstacle : l'écoulement y est celui du
  cylindre, dont la direction amont est imposée.
