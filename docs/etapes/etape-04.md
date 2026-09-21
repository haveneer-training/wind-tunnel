# Étape 4 — L'écoulement porteur

**Fichier :** `src/velocity.rs` · **Vérification :** `cargo xtask goto 4` puis `cargo test` · **≈ 20 min**

Le traceur ne se déplace pas tout seul : il est transporté. Il nous faut donc un champ de
vitesse — et, cette fois, un trait pour ne pas dépendre de la façon dont il est obtenu.

## L'idée : la fonction de courant

Le trait demande deux choses : une vitesse, et une **fonction de courant** `ψ`. La seconde
est celle que le solveur utilisera vraiment, et elle mérite qu'on s'y arrête.

Si un écoulement plan est à divergence nulle — `∂u_x/∂x + ∂u_y/∂y = 0`, ce qui est le cas
d'un fluide incompressible — alors il existe un champ scalaire `ψ` tel que

```text
u = (∂ψ/∂y, −∂ψ/∂x)
```

Ce n'est pas une astuce : c'est la condition d'intégrabilité. La divergence nulle dit
exactement que la quantité `u_x dy − u_y dx` est une différentielle exacte, et `ψ` en est
la primitive. Un écoulement qui ne conserve pas le volume n'a pas de fonction de courant.

Trois propriétés en découlent, et ce sont les trois dont le projet se sert :

- **Les lignes de niveau de `ψ` sont les lignes de courant.** En effet `u · ∇ψ =
  (∂ψ/∂y)(∂ψ/∂x) − (∂ψ/∂x)(∂ψ/∂y) = 0` : le fluide se déplace le long des `ψ = constante`.
  Une paroi imperméable est donc une ligne `ψ = constante` — l'étape 12 en fera son levier.
- **La différence de `ψ` entre deux points est le débit qui passe entre eux**, par unité de
  profondeur : `ψ(B) − ψ(A) = ∫ u·n dl`, le long de *n'importe quelle* courbe de A à B.
  D'où son unité, des m²/s, et d'où le fait qu'un débit ne dépende que des extrémités.

  ```text
       ψ = 2  ────────────────────────────────  ligne de courant
                        ▲
                        │  débit = 2 − 1 = 1, quel que soit le chemin
                        ▼
       ψ = 1  ────────────────────────────────  ligne de courant
  ```

- **`ψ` est définie à une constante près**, et à *une constante par composante de
  frontière*. Sur une veine sans obstacle, une seule constante, sans importance. Dès qu'il
  y a un obstacle — un trou dans le domaine — il en apparaît une seconde, indépendante :
  c'est elle qui répartit le débit entre le dessus et le dessous. Elle ne se déduit d'aucun
  calcul local, et l'étape 12 devra la choisir.

**La convention de signe n'est pas arbitraire.** Reprenons les notations du maillage. Une
face porte une arête `a → b` d'une cellule ; posons `d = b − a`. Sa longueur est
`L = ‖d‖` (`Face::length`) et sa normale **unitaire** est `n` (`Face::normal`). Les
sommets d'une cellule étant rangés dans le sens direct (`CellKind`), cette normale est `d`
tournée d'un quart de tour horaire puis normalisée (`Vec2::perp_cw`), donc sortante :

```text
n = perp_cw(d) / L        d'où      n L = perp_cw(d) = (d_y, −d_x)
```

Le débit sortant de la face — une vitesse fois une longueur, des m²/s, l'unité de `ψ` —
est le flux de `u` à travers elle, `u · n L` :

```text
u · n L = u_x d_y − u_y d_x             (car n L = (d_y, −d_x))
        = (∂ψ/∂y) d_y + (∂ψ/∂x) d_x     (car u = (∂ψ/∂y, −∂ψ/∂x))
        = ∇ψ · d = ψ(b) − ψ(a)
```

La dernière égalité est exacte dès que `ψ` varie linéairement le long de l'arête. Et c'est
de toute façon `ψ(b) − ψ(a)` que le solveur calcule : `compute_face_flux` ne fait qu'une
soustraction de `ψ` aux deux sommets, la vitesse n'apparaît nulle part.

Sur les deux écoulements de cette étape, la vérification est à la main :

- **`Uniform`** — avec `ψ(p) = u_x p_y − u_y p_x`, la différence `ψ(b) − ψ(a)` vaut
  `u_x d_y − u_y d_x`, soit exactement `u · n L`. Cas concret : une face verticale
  `d = (0, h)` traversée par `u = (U, 0)` donne un débit `U h > 0` — positif, donc
  sortant vers la droite, ce qu'on attend. Prenez `ψ` de signe opposé et tous les débits
  s'inversent.
- **`PotentialCylinder`** — sur la paroi `r = R`, `ψ` est constante, donc
  `ψ(b) − ψ(a) = 0` sur toute arête qui la suit : aucun débit ne traverse l'obstacle.
  C'est la même propriété que le « `u · n = 0` sur la paroi » que vérifie
  `flow_slips_along_the_cylinder_wall`, vue depuis `ψ` au lieu de `u`.

Inversez la rotation — un quart de tour trigonométrique au lieu d'horaire — et tout
l'écoulement s'inverse, de façon parfaitement conservative, donc silencieuse (mais le test 
`stream_function_matches_the_velocity` veille).

## Socle

| Bloc | Ce qu'il doit faire |
|---|---|
| `impl VelocityField for Uniform` | la vitesse et la fonction de courant d'un écoulement uniforme |
| `impl<F: VelocityField + ?Sized> StreamSource for F` | faire de tout champ de vitesse une source de `ψ` |

**`Uniform`** est l'écoulement le plus simple qui soit — la même vitesse partout — et
c'est exactement pourquoi il sert ici : sa vitesse s'écrit en un mot, mais sa fonction de
courant vous oblige à vous servir de `u = (∂ψ/∂y, −∂ψ/∂x)` plutôt qu'à recopier une
formule. Cherchez le `ψ` dont les deux dérivées partielles redonnent `value`, et fixez la
constante d'intégration à zéro. Une erreur de signe ou deux composantes échangées passent
inaperçues sur un écoulement horizontal ; le test prend une vitesse oblique exprès.

**L'implémentation couvrante de `StreamSource`** est le second trou, et il tient en une
ligne. Elle vaut pour *tout* type implémentant `VelocityField` — présent ou à venir —
sans que celui-ci ait un mot à écrire. Un écoulement analytique connaît `ψ` partout : le
numéro de sommet ne lui sert à rien, il répond au point.

Le `?Sized` de sa déclaration mérite un arrêt. Sans lui, la borne implicite `F: Sized`
exclut les types dont la taille n'est pas connue à la compilation — au premier rang
desquels `dyn VelocityField`, sous lequel le programme reçoit l'écoulement choisi par la
ligne de commande. Un `&dyn VelocityField` ne serait alors pas une `StreamSource`, et le
solveur le refuserait. Le test `any_velocity_field_is_already_a_stream_source` passe les
deux formes, concrète et `dyn`.

**`PotentialCylinder`, lui, vous est donné** : c'est la solution classique de
l'écoulement parfait incompressible irrotationnel, et la transcrire n'apprendrait rien
que l'algèbre ci-dessous ne dise déjà. En coordonnées cartésiennes relatives au centre,
avec `r² = x² + y²` :

```text
u =  U (1 − R²(x² − y²)/r⁴) − Γ y / (2π r²)
v = −U (2 R² x y / r⁴)      + Γ x / (2π r²)
```

`U` est la vitesse à l'infini, `R` le rayon, `Γ` la circulation. Lisez-le pour deux
choses : le garde-fou près du centre (les cellules y sont solides, mais `r⁴` au
dénominateur ne pardonne pas), et le fait que ses tests vérifient des propriétés
*physiques* — la vitesse tend vers `U` loin de l'obstacle, et sa composante normale est
nulle sur la paroi : le fluide glisse le long de l'obstacle sans le traverser.

## Ce qu'il y a à remarquer

**Un trait nomme un besoin, pas une implémentation.** Le solveur ne connaît que
`VelocityField` ; il ne sait pas s'il a affaire à une formule analytique ou, à l'étape 12,
au résultat d'un calcul sur le maillage. Ajouter un écoulement ne demandera aucune
modification du solveur — et l'étape 12 va plus loin dans la même direction : elle
s'aperçoit que le solveur n'a même pas besoin de tout `VelocityField`, seulement de `ψ`
aux sommets, et nomme ce besoin plus étroit `StreamSource`.

Un développeur C++ y verra une interface abstraite, en moins cher : quand le type est
connu à la compilation, l'appel est direct, sans table virtuelle, et souvent intégré à
l'appelant.

**`Sync` est exigé dès maintenant** dans la déclaration du trait, alors que rien n'est
encore parallèle. C'est la promesse qu'un champ de vitesse peut être lu depuis plusieurs
threads à la fois. En l'exigeant tout de suite, on s'évite de modifier le contrat à
l'étape 9 — et surtout, le compilateur refusera dès aujourd'hui une implémentation qui
ne pourrait pas l'être.

**`ψ` n'est pas là par élégance mathématique.** Le débit d'une face ne dépendant que de
ses deux extrémités, le bilan d'une cellule fermée est une somme télescopique — chaque
sommet y entre une fois avec un `+` et une fois avec un `−` — donc **nulle à la précision
machine**. Échantillonner la vitesse aux faces donnerait le même résultat à l'erreur de
quadrature près, et cette erreur-là ne pardonne pas.

L'enjeu est très concret. La première version de ce code échantillonnait la vitesse. La
divergence discrète valait alors ~10⁻³ au lieu de 0, et le schéma décentré — qui vérifie
pourtant un principe du maximum — fabriquait du traceur : partant de valeurs dans [0, 1],
le champ montait à **1,78**. Discrétiser le bon objet vaut mieux que rattraper l'erreur
ensuite.

## Pour aller plus loin

- Les tests `stream_function_matches_the_velocity` (le cylindre, donné) et
  `the_uniform_stream_function_matches_its_velocity` (le vôtre) vérifient par différences
  finies que `u = (∂ψ/∂y, −∂ψ/∂x)`. C'est ainsi qu'on attrape une erreur de signe : deux
  formulations du même objet, comparées l'une à l'autre.
- Faites varier `--circulation` et regardez la dissymétrie apparaître. Quel signe fait
  passer l'écoulement rapide au-dessus ?
- Implémentez un `VelocityField` de votre choix — un tourbillon, un point source — et
  passez-le au solveur. Combien de lignes ailleurs dans le code faut-il changer ?
