# Bonus — Chasse aux allocations

**Fichiers :** `src/solver.rs`, `src/mesh.rs` · **Prérequis :** étapes 7
et 8 · **≈ 45 min** · *bonus, transversal*

Ce bonus ne s'ouvre pas avec `cargo xtask goto` : il n'ajoute aucune fonctionnalité, il
reprend du code déjà écrit et lui retire ce qu'il alloue pour rien. C'est le complément
naturel des « pour aller plus loin » des étapes 7 et 8, et il vaut d'être fait en entier
au moins une fois — la discipline qu'il installe est celle de tous les codes de calcul.

## Pourquoi cela compte

Un `malloc` coûte quelques dizaines de nanosecondes, ce qui n'est rien. Le problème n'est
pas là :

- **Il ne parallélise pas.** L'allocateur système est un point de synchronisation entre
  threads. Une boucle `rayon` qui alloue par itération passe son temps à se battre pour
  un verrou que le code ne montre pas.
- **Il détruit les caches.** Un tampon réalloué à chaque pas de temps atterrit à une
  adresse différente à chaque fois : le processeur ne peut rien préfetcher, et les
  données utiles se font expulser par des pages fraîchement mappées.
- **Il n'est pas borné.** Un code qui alloue dans sa boucle en temps n'a pas d'empreinte
  mémoire prévisible. Sur un cluster où l'on réserve la mémoire par nœud à l'avance,
  c'est rédhibitoire.

D'où la règle, dans ce projet comme ailleurs : **la mémoire se réserve avant la boucle en
temps, jamais dedans**. `Solver::run` l'applique déjà pour son tampon `work` ; les
exercices ci-dessous étendent la règle à ce qui y échappe encore.

## Mesurer d'abord

Optimiser sans mesurer, c'est deviner. Rust permet de remplacer l'allocateur global par
un allocateur à soi : celui-ci délègue tout au système et se contente de compter au
passage. `examples/alloc_count.rs` le fait déjà ; le cœur tient en quinze lignes :

```rust
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;
```

C'est l'un des rares endroits où `unsafe` est incontournable : on promet au compilateur
de respecter un contrat (rendre un bloc aligné de la bonne taille, ou le pointeur nul)
qu'aucun type ne sait exprimer. Noter aussi que `#[global_allocator]` est *global* : il
s'applique à tout le binaire, bibliothèques comprises.

```shell
cargo run --release --example alloc_count
```

Sur `domains/tunnel.dom --refine 4` (90 016 cellules, 180 796 faces), l'état actuel du
corrigé :

| Phase | Allocations | Octets |
|---|---:|---:|
| `Mesh::from_mask` (une fois) | **270 149** | 75 Mo |
| 1 pas de temps Euler | 1 | 1,4 Mo |
| 1 pas de temps RK2 | 4 | 4,2 Mo |
| 1 fichier VTK | 3 | 8 Ko |
| 1 image PNG | 86 | 1 Mo |

Les deux dernières lignes valaient respectivement 273 691 et 270 104 allocations avant
que les sorties ne soient corrigées (voir « Ce qui est déjà fait », plus bas). Les trois
premières sont l'objet des exercices.

Relancez plusieurs fois : le nombre d'allocations des pas de temps varie de quelques
unités d'une exécution à l'autre — c'est `rayon` qui prépare ses files de travail au
premier appel, indépendamment du code du solveur. Le nombre d'**octets**, lui, est
parfaitement reproductible : c'est la colonne à regarder, et celle qui doit tomber à zéro
à la fin des exercices A et B.

## Exercice A — le tampon de gradients (étape 7)

`Solver::residual`, dans `src/solver.rs`, ouvre sur :

```rust
let mut gradients = vec![Vec2::ZERO; c.len()];
gradient::limited_gradients(self.mesh, c, &mut gradients);
```

Une allocation de 16 octets par cellule — 1,4 Mo ici — à chaque évaluation du résidu,
donc à chaque pas de temps. Sur `--max-steps 960`, cela fait 1,3 Go alloués et rendus pour un
tableau dont la taille ne change jamais.

`limited_gradients` prend déjà son `out` en paramètre, précisément pour que l'appelant
choisisse où il vit. Faites remonter le tampon jusqu'à `Solver::run`, qui possède déjà
`work`.

<details>
<summary>Solution</summary>

Un `Workspace` regroupe les tampons plutôt que d'allonger indéfiniment les signatures :

```rust
/// Les tampons de travail d'une boucle en temps, alloués une fois pour toutes.
pub struct Workspace {
    /// Résidu courant (`k1` en RK2).
    rate: Field,
    /// Gradients limités, une entrée par cellule.
    gradients: Vec<Vec2>,
}

impl Workspace {
    /// Réserve les tampons d'un calcul à `n` cellules.
    pub fn new(n: usize) -> Workspace {
        Workspace {
            rate: Field::zeros(n),
            gradients: vec![Vec2::ZERO; n],
        }
    }
}
```

`residual` reçoit les gradients au lieu de les allouer :

```rust
pub fn residual(&self, c: &Field, gradients: &mut [Vec2], out: &mut Field) {
    gradient::limited_gradients(self.mesh, c, gradients);
    // … la boucle sur les cellules ne change pas, `gradients` y est déjà un `&[Vec2]`
}
```

et `run` crée le `Workspace` avant la boucle :

```rust
pub fn run(
    &self,
    c: &mut Field,
    mut observer: impl FnMut(usize, f64, &Field) -> Result<(), SolverError>,
) -> Result<(), SolverError> {
    let mut work = Workspace::new(c.len());
    observer(0, 0.0, c)?;

    for step in 1..=self.config.steps {
        self.step(c, &mut work);
        // …
    }
    Ok(())
}
```

Vérification : relancez `alloc_count`. La ligne `20 pas Euler` compte aujourd'hui
28 130 Ko pour 20 pas, soit exactement les 1,4 Mo du `vec![Vec2::ZERO; …]` à chaque pas ;
le tampon remonté dans `run`, la colonne des octets tombe à zéro.

</details>

## Exercice B — les tampons de RK2 (étape 8)

`Solver::step_rk2` alloue deux champs de plus à chaque pas :

```rust
let mut predictor = c.clone();          // une allocation
// …
let mut k2 = Field::zeros(c.len());     // une deuxième
```

Ajoutez-les au `Workspace` de l'exercice A. Le point à retenir n'est pas la déclaration
mais le remplissage : `c.clone()` alloue **et** copie, alors que
[`Field::copy_from`](../src/field.rs) — qui n'attend que ce client — copie dans un tampon
déjà là.

<details>
<summary>Solution</summary>

```rust
pub struct Workspace {
    rate: Field,       // k1
    second: Field,     // k2, inutilisé en Euler
    predictor: Field,  // idem
    gradients: Vec<Vec2>,
}
```

```rust
fn step_rk2(&self, c: &mut Field, work: &mut Workspace) {
    self.residual(c, &mut work.gradients, &mut work.rate);

    work.predictor.copy_from(c);
    for (value, k1) in work
        .predictor
        .as_mut_slice()
        .iter_mut()
        .zip(work.rate.as_slice())
    {
        *value += self.dt * k1;
    }

    self.residual(&work.predictor, &mut work.gradients, &mut work.second);
    // … la combinaison finale ne change pas
}
```

Le compilateur refuse `self.residual(&work.predictor, &mut work.gradients, …)` si les
trois champs sont pris à travers `work` en même temps — c'est le même emprunt, une fois
partagé et une fois exclusif. Déstructurer d'abord (`let Workspace { predictor, gradients,
second, .. } = work;`) lève l'objection sans rien copier : le compilateur suit les champs
un par un, contrairement au C où rien n'aurait été vérifié du tout.

Vérification : la ligne `20 pas Rk2` compte aujourd'hui 4,2 Mo par pas — deux tampons de
gradients (deux évaluations du résidu), le prédicteur, `k2`. Les quatre disparaissent, et
la colonne des octets tombe à zéro comme en Euler.

Deux champs de plus sont alors alloués même en Euler, où ils ne servent pas. C'est un
choix : quelques mégaoctets réservés une fois, contre un `Option<…>` à déballer à chaque
pas. Sur un cas réel, la mémoire résidente est le critère — si elle devient un problème,
c'est que le `Workspace` doit dépendre du schéma, pas qu'il faut réallouer.

</details>

## Exercice C — la construction du maillage

`Mesh::from_mask` alloue trois `Vec` par cellule :

```rust
let corners = cell_corners(row, col, down, right, up, left);   // Vec<(usize, usize)>
let ids: Vec<VertexId> = corners.iter().map(…).collect();      // Vec<VertexId>
let points: Vec<Point> = ids.iter().map(…).collect();          // Vec<Point>
```

soit 270 000 allocations et 75 Mo pour 90 000 cellules. C'est une fois pour toutes, donc
sans commune mesure avec les exercices A et B — mais c'est le cas d'école du `Vec`
employé là où la taille est connue et minuscule : une cellule a trois ou quatre sommets,
jamais plus.

Remplacez les trois par des tableaux de taille fixe. Attention : `cell_corners` est le
trou de l'étape 2, la version à `vec![…]` reste la bonne réponse à cette étape-là.

<details>
<summary>Solution</summary>

Un petit type « tableau plus longueur » — ce que fournirait la crate `arrayvec`, en dix
lignes et sans dépendance :

```rust
/// Trois ou quatre coins, sans allocation.
struct Corners {
    points: [(usize, usize); 4],
    len: usize,
}

impl Corners {
    fn tri(a: (usize, usize), b: (usize, usize), c: (usize, usize)) -> Corners {
        Corners { points: [a, b, c, a], len: 3 }
    }

    fn quad(a: (usize, usize), b: (usize, usize), c: (usize, usize), d: (usize, usize)) -> Corners {
        Corners { points: [a, b, c, d], len: 4 }
    }

    fn as_slice(&self) -> &[(usize, usize)] {
        &self.points[..self.len]
    }
}
```

`cell_corners` renvoie `Corners` et remplace `vec![bl, tr, tl]` par `Corners::tri(bl, tr,
tl)`. Côté appelant, deux tableaux locaux suffisent :

```rust
let corners = cell_corners(row, col, down, right, up, left);
let n = corners.as_slice().len();
let mut ids = [VertexId(0); 4];
let mut points = [Point::new(0.0, 0.0); 4];
for (k, &(vr, vc)) in corners.as_slice().iter().enumerate() {
    ids[k] = vertex_id(vr, vc, rows, cols, h, &mut vertices, &mut vertex_table);
    points[k] = vertices[ids[k].index()];
}
let (ids, points) = (&ids[..n], &points[..n]);
```

`polygon_area` et `polygon_centroid` prennent un `&[Point]` : ils acceptent le sous-tableau
sans rien savoir de ce changement. C'est l'intérêt d'avoir écrit les signatures en
tranches plutôt qu'en `Vec`.

Vérification : `Mesh::from_mask` passe de 270 149 à **101 allocations**. Les 62 Mo
restants sont la croissance par doublement de `vertices`, `cells`, `faces` et de la
`HashMap` d'arêtes ; les réserver (`Vec::with_capacity`, `HashMap::with_capacity`)
descend à 41 allocations et 52 Mo, pour un gain de temps négligeable — la limite de
l'exercice est atteinte, et c'est aussi une leçon.

</details>

## Ce qui est déjà fait dans le corrigé

Trois chasses du même genre sont déjà appliquées ; elles se lisent comme des exemples
corrigés.

- **`VtkF64`, dans `src/io/vtk.rs`.** Écrire un flottant passait par une `String` jetée
  aussitôt, soit une allocation par nombre et 273 691 par fichier. Le type implémente
  `Display` et se formate directement dans le tampon de sortie : 3 allocations par
  fichier. C'est l'usage normal de `Display` en Rust — *savoir s'écrire*, et non
  *fabriquer une chaîne* — et il n'a aucun équivalent simple en C, où renvoyer une chaîne
  formatée oblige à choisir entre un tampon statique et un `malloc`.
- **Les tampons de `write_png`, dans `src/io/png.rs`.** Le polygone, ses coins et la liste
  de pixels d'une cellule étaient réalloués 90 016 fois par image. Sortis de la boucle et
  vidés par `clear()`, ils gardent leur capacité d'une cellule à l'autre : 270 104 → 86
  allocations. Dans la version parallèle de l'étape 10, ils sont déclarés *par thread*,
  ce qui est aussi ce qui les garde privés — aucun partage, donc aucun verrou.
- **`Halo`, dans `mpi/src/exchange.rs`.** Les quatre tampons d'un échange sont alloués une
  fois par rang au lieu d'une fois par échange, soit deux fois par pas de temps en RK2.
  Leur taille ne dépend que du découpage, qui ne bouge plus après le démarrage.

## Ce qu'il ne faut pas en conclure

Que toute allocation est un défaut. `Vec`, `String` et `Box` sont les bons outils par
défaut, et un code qui les fuit par principe devient illisible pour un gain nul. Ce qui
compte est **où** l'allocation se produit :

- dans une boucle en temps, dans une boucle sur les cellules, dans une région
  parallèle : à supprimer ;
- au montage du cas, à la lecture des options, dans un message d'erreur : parfaitement
  légitime, et le contraire serait de la coquetterie.

Le tableau de `alloc_count` sépare les deux d'un coup d'œil ; c'est pour cela qu'il vient
avant les exercices, et non après.
