# Étape 0 — Géométrie de base

**Fichier :** `src/geom.rs` · **Vérification :** `cargo test geom` · **≈ 20 min**

Avant le maillage, les briques : un point, un vecteur, l'aire et le centre de gravité
d'un polygone. Ce sera la fondation de tout le reste, et l'occasion de voir à quoi
ressemble du Rust ordinaire.

## Socle

Quatre trous à combler, tous d'une ou deux lignes :

| Fonction | Ce qu'elle doit faire |
|---|---|
| `Vec2::dot` | produit scalaire |
| `Vec2::perp_cw` | rotation de −90°, pour obtenir une normale sortante |
| `polygon_area` | aire signée par la formule du lacet : `½ Σ (xᵢ·yᵢ₊₁ − xᵢ₊₁·yᵢ)` |
| `polygon_centroid` | centroïde : `(1/6A) Σ (pᵢ + pᵢ₊₁)·(xᵢ·yᵢ₊₁ − xᵢ₊₁·yᵢ)` |

La fonction utilitaire `edges` vous donne déjà les couples `(sommet, sommet suivant)` en
refermant le polygone : vous n'avez pas à gérer le retour au premier sommet.

## Ce qu'il y a à remarquer

**`Point` et `Vec2` sont deux types différents**, alors qu'ils portent les mêmes deux
`f64`. C'est délibéré. Soustraire deux points donne un déplacement (`Point - Point ->
Vec2`), ajouter un déplacement à un point donne un point (`Point + Vec2 -> Point`), mais
additionner deux points n'a aucun sens — et n'est donc pas implémenté. En C, les deux
seraient le même `struct { double x, y; }` et l'erreur passerait sans un mot.

**Le signe de l'aire porte une information** : positif si les sommets tournent dans le
sens direct. Le maillage s'en servira pour détecter une cellule retournée.

**`#[derive(Clone, Copy, Debug, PartialEq)]`** au-dessus des structures fait écrire au
compilateur la copie, l'affichage de mise au point et la comparaison. `derive` reviendra
souvent.

## Pour aller plus loin

- Les exemples dans les commentaires `///` sont exécutés par `cargo test` : ce sont des
  *doc-tests*. Ajoutez-en un à `Vec2::norm`, et vérifiez qu'il tourne.
- Que renvoie `polygon_area` pour un polygone croisé en forme de 8 ? Pourquoi ?
- `polygon_centroid` traite à part le cas d'une aire négligeable. Sans ce garde-fou, que
  renverrait-il ?
