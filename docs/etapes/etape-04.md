# Étape 4 — L'écoulement porteur

**Fichier :** `src/velocity.rs` · **Vérification :** `cargo xtask goto 4` puis `cargo test` · **≈ 25 min**

Le traceur ne se déplace pas tout seul : il est transporté. Il nous faut donc un champ de
vitesse — et, cette fois, un trait pour ne pas dépendre de la façon dont il est obtenu.

## Socle

**`PotentialCylinder::at`** — l'écoulement potentiel autour d'un cylindre, solution
classique de l'écoulement parfait incompressible irrotationnel. En coordonnées
cartésiennes relatives au centre, avec `r² = x² + y²` :

```text
u =  U (1 − R²(x² − y²)/r⁴) − Γ y / (2π r²)
v = −U (2 R² x y / r⁴)      + Γ x / (2π r²)
```

`U` est la vitesse à l'infini, `R` le rayon, `Γ` la circulation. Renvoyez une vitesse
nulle très près du centre : les cellules y sont solides, mais `r⁴` au dénominateur ne
pardonne pas.

Les tests vérifient les deux propriétés physiques qui comptent : la vitesse tend vers `U`
loin de l'obstacle, et sa composante normale est nulle sur la paroi du cylindre — le
fluide glisse le long de l'obstacle sans le traverser.

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

**La fonction de courant `ψ` n'est pas là par élégance.** Le trait demande aussi
`stream`, déjà écrite. Le débit à travers une arête `a → b` vaut exactement `ψ(b) − ψ(a)` ;
en calculant les débits ainsi plutôt qu'en échantillonnant la vitesse aux faces, le bilan
de débit d'une cellule fermée devient une somme télescopique, **nulle à la précision
machine**.

L'enjeu est très concret. La première version de ce code échantillonnait la vitesse. La
divergence discrète valait alors ~10⁻³ au lieu de 0, et le schéma décentré — qui vérifie
pourtant un principe du maximum — fabriquait du traceur : partant de valeurs dans [0, 1],
le champ montait à **1,78**. Discrétiser le bon objet vaut mieux que rattraper l'erreur
ensuite.

## Pour aller plus loin

- Le test `la_fonction_de_courant_redonne_la_vitesse` vérifie par différences finies que
  `u = (∂ψ/∂y, −∂ψ/∂x)`. C'est ainsi qu'on attrape une erreur de signe : deux
  formulations du même objet, comparées l'une à l'autre.
- Faites varier `--circulation` et regardez la dissymétrie apparaître. Quel signe fait
  passer l'écoulement rapide au-dessus ?
- Implémentez un `VelocityField` de votre choix — un tourbillon, un point source — et
  passez-le au solveur. Combien de lignes ailleurs dans le code faut-il changer ?
