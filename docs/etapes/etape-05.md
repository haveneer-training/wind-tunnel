# Étape 5 — Le solveur

**Fichiers :** `src/flux.rs`, `src/solver.rs` · **Vérification :** `cargo xtask goto 5`, `cargo test`, puis `cargo run --release -- domains/tunnel.dom` · **≈ 45 min**

Tout est en place : un maillage, un écoulement, des sorties. Il reste à faire avancer le
traceur — et, à la fin de cette étape, à regarder l'image.

## Le schéma

Sur chaque cellule, on écrit le bilan de ce qui entre et sort :

```text
|Ωi| · dcᵢ/dt = − Σ_f [ q_f · c_face − D (c_voisin − cᵢ) L_f / d_f ]
```

La somme porte sur les faces de la cellule `i`, et chaque symbole a son pendant dans le
code :

| Symbole | Ce que c'est | Dans `residual` |
|---|---|---|
| `\|Ωi\|` | aire de la cellule (le calcul est plan, tout est par unité de profondeur) | `cell.area` |
| `q_f` | le débit de la face, **compté sortant de la cellule `i`** — celui de l'étape 4, `ψ(b) − ψ(a)`, en m²/s | `self.face_flux[fid] * outward` |
| `c_face` | la valeur du traceur emportée à travers la face | `self.scheme.interface_value(…)` |
| `c_voisin` | la valeur de l'autre côté : celle de la cellule voisine, ou celle qu'impose la condition de bord | `c[neighbour]` ou `self.bc(kind)` |
| `D` | la diffusivité (`--diffusivity`, nulle par défaut) | `self.config.diffusivity` |
| `L_f`, `d_f` | longueur de la face, distance entre les deux centres de cellules | `face.length`, `face.distance` |

Deux termes de natures différentes : `q_f · c_face` est l'**advection** — le fluide
emporte ce qu'il contient —, et `D (c_voisin − cᵢ) L_f / d_f` la **diffusion**, le
gradient normal approché par une différence entre deux centres. Le tout est sommé face
par face dans `sum`, puis `out[id] = -sum / cell.area` : le signe `−` et la division par
l'aire sont ceux de la formule.

Le signe de `q_f` se règle une fois pour toutes : `face_flux` est orienté sortant de
`face.left` (étape 4), donc la cellule qui se trouve de l'autre côté le retourne —
c'est tout le rôle de `outward`. Une face vue des deux cellules donne ainsi deux débits
exactement opposés : ce qui sort de l'une entre dans l'autre, à la précision machine.

`c_face`, lui, est la seule chose que le schéma décide, et c'est le trou de `flux.rs`.
Le décentrement amont prend la valeur du côté d'où vient le fluide — le signe de
`un = q_f / L_f`, la vitesse normale que `FaceState` transporte. Simple, jamais
oscillant, et diffusif : un front net s'étale en traversant le domaine. C'est
précisément le défaut que l'étape 7 corrigera.

## Le pas de temps maximal

L'intégration est explicite : `c` à l'instant suivant ne se lit que sur l'instant
courant, sans système à résoudre — mais au prix d'un pas de temps borné.

```text
dt ≤ min_i  |Ωi| / Σ_f ( |q_f| + 2·D·L_f/d_f )
```

Vérification d'unités d'abord : `q_f` est en m²/s (étape 4), `D L/d` aussi (des m²/s
fois un rapport de longueurs sans dimension). La somme est donc en m²/s, l'aire en m²,
et le rapport en **secondes**.

D'où vient la borne ? Écrivons un pas d'Euler explicite avec le décentrement amont, en
regroupant les termes sur `cᵢ`. En notant `q⁺` la part sortante d'un débit et `q⁻` sa
part entrante (`q = q⁺ − q⁻`, toutes deux positives) :

```text
cᵢⁿ⁺¹ = cᵢ [ 1 − (dt/|Ωi|) Σ_f (q⁺_f + D L/d) ] + (dt/|Ωi|) Σ_f (q⁻_f + D L/d) c_voisin
```

Tous les coefficients des voisins sont positifs, et leur somme avec celui de `cᵢ` vaut
exactement `1` — parce que `Σ_f q_f = 0` sur une cellule fermée, la propriété
télescopique de l'étape 4. La nouvelle valeur est donc une **moyenne pondérée** des
anciennes, donc comprise entre leur minimum et leur maximum — le champ reste dans
`[0, 1]` — **à la condition** que le coefficient de `cᵢ` soit lui aussi positif :

```text
dt ≤ |Ωi| / ( Σ_f q⁺_f + D Σ_f L/d )
```

C'est le vrai critère. Celui qu'implémente `max_stable_dt` est **deux fois plus
sévère** : comme `Σ q_f = 0`, on a `Σ|q_f| = 2 Σ q⁺_f`, et le `2·D` fait de même sur le
terme diffusif. Cette marge d'un facteur 2 se vérifie à la main sur la veine vide, où
une cellule intérieure est un carré de côté `h` traversé par `U` : deux faces portent
`|q| = U h`, les deux autres rien, donc `dt_max = h² / (2 U h) = h / 2U`. Avec les
valeurs par défaut (`h = 1`, `U = 1`), le programme annonce bien `maximum stable
5.0000e-1 s` — la moitié de la limite de positivité.

Écrire la somme avec `|q_f|` plutôt qu'avec `q⁺_f` a un second avantage : la formule ne
suppose plus rien sur `Σ q_f`. Elle reste donc valable — conservative — sur une cellule
de bord dont une face est en `NoFlux`, que `residual` ignore mais que cette somme compte
quand même.

Restent deux mots sur la forme de l'expression :

- **`min_i`** : le pas de temps est global, le même pour toutes les cellules. C'est donc
  la plus contraignante qui l'impose. Avec de la diffusion, ce sont les cellules de coin :
  sur une face de bord, `d` est la distance du centre au *milieu de la face*
  (`finish_geometry`), soit deux fois moins qu'entre deux centres voisins — et un coin en
  a deux.
- **`Solver::new` en fait deux choses** : sans `--dt`, il prend `config.cfl * dt_max`
  (`cfl = 0,4` par défaut, donc un cinquième de la limite de positivité) ; avec `--dt`,
  il compare et refuse par `SolverError::Cfl` plutôt que de laisser diverger.

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
aussi une collision de données dès qu'on la parallélise : deux threads traitant deux faces
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
cargo run --release -- domains/tunnel.dom --max-steps 400 --every 20 --circulation 4
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
  des concentrations négatives, dont l'amplitude croît. Poussez à `--max-steps 4000` et le
  champ atteint 10¹⁶. Le décentrement amont, lui, reste rigoureusement borné. « Ordre
  plus élevé » et « meilleur » sont deux choses différentes, et c'est exactement le
  problème que devra résoudre l'étape 7.
- Raffinez avec `--refine 4` : chaque case du masque est subdivisée en 4×4, soit
  90 000 cellules au lieu de 5 600, sur le **même** domaine physique. L'interface entre
  bandes s'affine et le pas de temps stable est divisé par quatre. Chronométrez : c'est
  le sujet des étapes 9 et 10.
- Attention à ne pas confondre avec `--cell-size`, qui fixe la *taille* d'une cellule et non
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
