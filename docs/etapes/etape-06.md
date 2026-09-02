# Étape 6 — Qualité : conservation, ordre, clippy

**Fichiers :** `src/field.rs` · **Vérification :** `cargo xtask goto 6`, `cargo test`, puis `cargo clippy --all-targets --all-features -- -D warnings` · **≈ 30 min**

Le code tourne depuis l'étape 5 et produit ses premières images. Cette étape ne lui ajoute
rien : elle vérifie ce qu'il fait déjà, de deux façons différentes, et fait le ménage dans
ce que vous avez écrit.

## La conservation, déjà vérifiée

`src/solver.rs` contient déjà deux tests de conservation : `face_fluxes_are_divergence_free`
(le bilan de débit d'une cellule est nul à la précision machine) et
`pure_diffusion_conserves_mass` (sur un domaine fermé, la masse totale est un invariant
exact). Relisez-les : ce sont des tests de non-régression bien plus sévères qu'une capture
d'écran, parce qu'ils vérifient une propriété *physique* du schéma, pas un résultat figé.

## L'ordre, à mesurer

Une propriété qu'aucun test existant ne couvre : l'**ordre de convergence** du schéma —
à quelle vitesse l'erreur diminue quand le maillage se raffine. On la mesure par
**solution manufacturée** : un profil lisse (une gaussienne), transporté par un écoulement
uniforme. Sans terme source à ajouter au solveur, parce que l'écoulement est à divergence
nulle : la solution exacte de l'advection pure est alors sa propre translation,

```text
c_exact(x, y, T) = c₀(x − U·T, y)
```

`tests/order.rs` construit ce cas à deux niveaux de raffinement du même domaine physique
(`Mask::refine`, comme `--refine` en CLI), roule chacun jusqu'à la même durée physique, et
compare le champ numérique à sa translation exacte. L'écart entre les deux a besoin d'une
mesure : c'est le trou de cette étape.

## Socle

| Fonction | Fichier | Ce qu'elle doit faire |
|---|---|---|
| `Field::mean_abs_error` | `field.rs` | écart moyen entre deux champs, pondéré par l'aire des cellules : `Σ |cᵢ − dᵢ| · |Ωᵢ| / Σ |Ωᵢ|` |

Deux tests unitaires dans `field.rs` la vérifient directement. Une fois qu'elle est
écrite, `cargo test` fait tourner `order_of_convergence_is_about_one`, qui calcule
l'ordre observé entre les deux raffinements : `ln(e₁/e₂) / ln(2)`.

## Ce qu'il y a à remarquer

**L'ordre mesuré doit valoir ≈ 1 — c'est voulu, pas un bug.** Le décentrement amont est
d'ordre 1 en espace, et l'intégration en temps est un Euler explicite, d'ordre 1 lui
aussi. Le test fixe le nombre de Courant (le pas de temps est déduit de `solver.dt()` à
chaque raffinement) plutôt que le nombre de pas : `dt` décroît donc proportionnellement à
`h`, et les deux erreurs — spatiale et temporelle — décroissent à la même vitesse. C'est
cette mécanique qui explique, à l'étape 8, pourquoi l'ordre mesuré **ne bougera pas**
après le passage à l'ordre 2 en espace de l'étape 7 : l'erreur en temps domine dès que
`dt ∝ h`, quel que soit l'ordre du schéma spatial.

**Le ménage, maintenant.** Une fois l'étape verte, passez le peigne sur tout ce que vous
avez écrit depuis l'étape 0 :

```shell
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo doc --no-deps --open
```

`clippy` repère ce qu'un compilateur silencieux laisse passer — allocations inutiles,
comparaisons redondantes, `.clone()` qu'on aurait pu éviter. `cargo doc` fait apparaître
votre nouvelle méthode dans la documentation générée : relisez-la comme le ferait
quelqu'un qui n'a jamais vu le code.

## Pour aller plus loin

- Ajoutez un troisième niveau de raffinement (`factor = 4`, en plus de 8 et 16) et
  vérifiez que l'ordre observé entre 4→8 et celui entre 8→16 sont proches : c'est la
  signature d'un régime asymptotique, plutôt qu'une coïncidence sur une seule paire de
  points.
- Changez `Upwind` en `Centered` dans `tests/order.rs` (schéma d'ordre 2, mais oscillant —
  voir l'étape 5). L'ordre mesuré s'améliore-t-il vraiment ? Le champ reste-t-il seulement
  borné pendant la mesure ?
