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
| `max_stable_dt` | `solver.rs` | la formule CFL ci-dessus |
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
- Divisez `--h` par deux : quatre fois plus de cellules, deux fois plus de pas de temps.
  Chronométrez. C'est le sujet des étapes 9 et 10.
