# Étape 12 — Calculer l'écoulement : Laplace, Jacobi, convergence

**Fichier :** `src/stream.rs` · **Vérification :** `cargo xtask goto 12`, `cargo test`
· **≈ 40 min** · *bonus*

Dessinez un carré dans le masque, lancez le calcul, regardez la fumée : elle contourne un
cercle. Depuis l'étape 4, l'écoulement porteur est une formule — celle du cylindre — et
`obstacle()` se contente de réduire la forme dessinée à un disque de même aire. La forme,
personne ne la voit.

Le second défaut est plus discret et plus gênant. Une paroi en escalier n'est pas une
ligne de courant de cette formule : un petit débit la traverse. C'est pour cela que les
parois sont en `ZeroGradient` et non en `NoFlux` (voir `Config::default`) — interdire ce
débit fabriquerait une divergence artificielle et le schéma perdrait sa borne. **Les deux
défauts ont la même cause : l'écoulement ignore le maillage.** Cette étape le calcule
dessus.

## L'idée

On ne résout pas la vitesse, on résout la fonction de courant `ψ` — celle dont le solveur
déduit tous ses débits depuis l'étape 4. Un écoulement plan, incompressible et
irrotationnel vérifie

```text
∇²ψ = 0
```

avec `ψ` **constante le long de chaque paroi** : c'est la définition d'une ligne de
courant. Voici la veine, vue comme un problème de Dirichlet :

```text
                ψ = Q  (paroi haute)
   ┌───────────────────────────────────────┐
   │                                       │
ψ = ψ₀(y)          ┌─────┐                 │   ∂ψ/∂x = 0
(entrée)           │ψ = c│                 │   (sortie libre)
   │               └─────┘                 │
   │                                       │
   └───────────────────────────────────────┘
                ψ = 0  (paroi basse)
```

`ψ` vit **aux sommets**, là où le solveur la lit. Chaque face interne d'extrémités
`(a, b)` devient une arête de poids `w = face.distance / face.length`, et l'équation du
sommet `v` s'écrit `Σ w (ψ_voisin − ψ_v) = 0`. Sur une grille de carrés, `w = 1` et c'est
le laplacien à cinq points ; sur les arêtes diagonales des triangles chanfreinés, le poids
est plus faible. Aucune matrice n'est stockée : le voisinage pondéré *est* la matrice.

Un balayage de Jacobi remplace chaque valeur libre par la moyenne pondérée de ses voisins,
et on recommence jusqu'à ce que le résidu tombe sous la tolérance.

## Le piège à éviter

**Croire que la conservation dépend de la convergence.** Elle n'en dépend pas du tout.

Le débit d'une face vaut `ψ(b) − ψ(a)`. Le bilan d'une cellule fermée est donc une somme
télescopique : chaque sommet y entre une fois avec un `+` et une fois avec un `−`. Le
résultat est exactement nul — pour *n'importe quel* champ `ψ` aux sommets, convergé ou
non, physique ou non. Arrêtez Jacobi après un seul balayage : l'écoulement sera absurde,
et le schéma restera parfaitement conservatif et borné.

La même remarque explique l'étanchéité : si les sommets d'une paroi portent la *même*
valeur, le débit de chacune de ses faces est `x - x`, soit `0.0` — pas « petit », pas
« à 1e-15 près » : zéro, au sens de l'arithmétique flottante. C'est ce que vérifie
`walls_are_exact_streamlines`, avec un `assert_eq!` et non une tolérance.

La conservation vient de la *forme* du schéma. La convergence, elle, ne décide que de la
justesse physique du résultat. Ce sont deux propriétés indépendantes, et c'est une
distinction qui manque à beaucoup de codes.

## Socle

| Bloc | Fonction | Ce qu'il doit devenir |
|---|---|---|
| 1 | `adjacency` | Le voisinage pondéré des sommets au format CSR : compter les arêtes de chaque sommet (faces internes seulement, chacune comptant pour ses **deux** extrémités), cumuler en offsets, puis remplir en avançant un curseur par sommet. Poids `w = distance / length`. Même schéma que `Mesh::build_cell_faces` |
| 2 | `boundary_values` | Un `Option<f64>` par sommet : `ψ₀(y) = speed × (y − ymin)` sur `Inlet` et `Wall`, **une constante** sur `Obstacle` (`ψ₀` de l'ordonnée moyenne de son contour), `None` sur `Outlet`. Les groupes viennent de `mesh.groups()` |
| 3 | `jacobi` | La boucle : un balayage `ψ_v ← (Σ w ψ_voisin) / (Σ w)` sur les sommets libres, écrit dans un second tampon puis échangé — jamais sur place ; le résidu par `residual_inf` ; l'arrêt sous `options.tol` ; `SolverError::NotConverged` si le budget s'épuise |

Ne rien assembler sur les faces de bord suffit à imposer la sortie libre : un sommet de
sortie n'a plus qu'un voisin, son équation devient `ψ_v = ψ_voisin`, c'est-à-dire
`∂ψ/∂x = 0`, donc une sortie horizontale. La condition de Neumann homogène s'obtient en ne
faisant rien — vérifiez-le sur le maillage.

## Ce qu'il y a à remarquer

```shell
cargo test --test stream
cargo run --release -- domains/square.dom --refine 2 --steps 200                  # analytique
cargo run --release -- domains/square.dom --refine 2 --steps 200 --flow computed  # calculé
```

- `walls_are_exact_streamlines` teste `assert_eq!(worst, 0.0)`. Un test d'égalité sur des
  flottants est d'ordinaire une faute ; ici c'est la propriété elle-même, et une tolérance
  l'affaiblirait.
- `the_analytic_flow_leaks_through_a_square_obstacle` verrouille l'inverse : la formule,
  elle, fuit. C'est la raison d'être de l'étape, écrite en test.
- `SolverError::NotConverged { iters, residual }` existait depuis l'étape 5, sans jamais
  être construit. Une variante d'erreur réservée d'avance pour un algorithme qui n'existait
  pas encore : c'est le genre de dette que le `match` exhaustif de Rust rend visible et
  inoffensive.
- Le **résidu n'est pas l'erreur**. Pour Jacobi, le premier vaut environ `(1 − ρ)` fois la
  seconde, et `1 − ρ` se dégrade comme le carré de la taille du maillage : un résidu à
  1e-10 ne garantit qu'une erreur à 1e-7 sur une petite veine, bien moins sur une grande.
  Le test `jacobi_converges_to_the_uniform_flow` mesure l'erreur vraie, pas le résidu.
- `ComputedStream` n'implémente **pas** `VelocityField` : il ne connaît `ψ` qu'aux sommets
  et ne saurait pas répondre en un point quelconque. Il implémente `StreamSource`, le
  contrat plus pauvre que le solveur consomme réellement. Un trait décrit un besoin, pas
  une capacité — c'est l'occasion de relire l'étape 4 sous cet angle.

## Pour aller plus loin

- **Mesurez la lenteur de Jacobi.** Relevez le nombre de balayages annoncé par `cargo run`
  à `--refine 1`, `2`, puis `4` : environ 7 100, 23 600 et 75 200 sur `domains/tunnel.dom`.
  Chaque doublement du raffinement le multiplie par un peu plus de trois — la théorie en
  prédit quatre, puisque le facteur de convergence vaut `1 − O(h²)`. Où cela devient-il
  inutilisable ? Et que se passe-t-il au-delà de `--stream-iters` (200 000 par défaut) ?
- **Écrivez un gradient conjugué matrix-free** (une trentaine de lignes) : le produit
  matrice-vecteur est déjà là, c'est le corps du balayage. Comptez les itérations — quelques
  centaines au lieu de plusieurs dizaines de milliers, pour le même résidu. Comparez aussi à
  un SOR (`ψ_v ← ψ_v + ω (moyenne − ψ_v)`, `ω ≈ 1,9`) : bien plus rapide que Jacobi, mais
  intrinsèquement séquentiel — le résultat dépend de l'ordre de parcours. Contraste utile
  avec la forme *gather* parallélisable du reste du projet.
- **Ne réinventez pas la roue** : `cargo run --release --example stream_faer` résout le même
  système avec le crate [`faer`](https://docs.rs/faer), par factorisation de Cholesky creuse
  — directe, donc sans tolérance ni itérations. Lisez l'exemple, chronométrez, et demandez-vous
  ce que le socle vous a appris que l'appel de bibliothèque n'apprend pas.
- **La constante de l'obstacle est arbitraire.** `ψ₀` de son ordonnée moyenne est exact pour
  une forme symétrique, approché sinon : cette constante répartit le débit entre le dessus et
  le dessous. La bonne condition est *circulation nulle autour du corps*. Le problème étant
  linéaire, elle s'obtient par superposition de deux résolutions — écrivez-la, et regardez la
  différence sur un obstacle placé hors de l'axe.
- **Plusieurs obstacles** demandent une constante par corps, donc un étiquetage des
  composantes connexes du masque (l'extension de l'étape 1, `Mask::check_connected`, en donne
  la moitié). Avec une seule constante pour deux corps, que se passe-t-il ?
- **Sous MPI**, l'exécutable de l'étape 11 refuse `--flow computed` : aucun rang ne détient le
  maillage complet. Faites-le marcher — c'est un gradient conjugué distribué, avec un échange
  de halo à chaque produit matrice-vecteur (le `Halo` existe déjà) et une réduction globale à
  chaque produit scalaire (sur le modèle de `global_dt_max`).
- **Ce n'est toujours qu'un écoulement potentiel** : pas de couche limite, pas de
  décollement, pas de sillage, pas de traînée, et une fumée symétrique amont/aval. Le modèle
  qui les donne est `ψ`–`ω` : la même équation avec `∇²ψ = −ω`, et une équation de transport
  pour `ω`… que le solveur du projet sait déjà résoudre.
