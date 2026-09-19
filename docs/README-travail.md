# Soufflerie numérique — projet fil rouge

Formation **« Découverte du langage Rust pour le calcul scientifique »**.

![filets de fumée déviés par un cylindre](docs/apercu.png)

Voilà ce que vous aurez produit à la fin de l'étape 5
(`cargo run --release -- domains/tunnel.dom --refine 4 --bands 9 --max-steps 960`).

Vous allez construire, étape par étape, un petit code de calcul complet : lecture d'un
domaine, génération d'un maillage non structuré, transport d'un traceur passif autour
d'un obstacle, écriture des résultats en VTK et en PNG.

Le squelette est là, les tests aussi. Ce qui manque, ce sont **30 `todo!()`**, répartis
sur treize étapes (0 à 12). Le code complet, lui, se trouve dans le répertoire parent, celui d'où
vous avez lancé `cargo xtask starter` : consultez-le quand vous voulez, mais essayez
d'abord.

Tout ce qui suit se passe **dans ce dossier-ci**.

## La boucle de travail

```shell
cargo test                  # rouge : voici ce qu'il faut écrire
#   ... ouvrir le fichier indiqué, remplacer le todo!() ...
cargo test                  # vert : étape terminée
cargo xtask goto 1          # étape suivante
cargo xtask status          # à tout moment : où j'en suis
```

`cargo test` ne montre jamais que les étapes déjà ouvertes : vous n'avez pas cinquante
tests rouges sous les yeux, seulement ceux qui vous concernent. Chaque échec nomme le
fichier et la ligne à compléter.

L'énoncé de chaque étape est dans [`docs/etapes/`](docs/etapes/) — commencez toujours par
le lire, il explique le *pourquoi* autant que le *quoi*. Le sommaire est dans
[`ETAPES.md`](ETAPES.md).

Les **avertissements** du compilateur suivent la même règle. Un `todo!()` rend
mécaniquement inutilisés les paramètres de sa fonction ; ceux des étapes que vous n'avez
pas encore ouvertes sont tus, pour que vous ne lisiez que les vôtres. Ceux qui restent
disent quelque chose d'utile : « paramètre `points` inutilisé » sur la fonction que vous
êtes en train d'écrire, c'est la liste de ce qu'il vous reste à employer. Ils
disparaissent quand l'étape est finie.

## Les commandes

| Commande | Effet |
|---|---|
| `cargo xtask status` | avancement étape par étape |
| `cargo xtask goto <n>` | passer à l'étape n ; remplit au passage les trous des étapes précédentes **restés vides** |
| `cargo xtask solve <n>` | remplir les trous de l'étape n à votre place (rattrapage) |
| `cargo xtask reset <n>` | rouvrir les trous de l'étape n pour la refaire |
| `cargo test` | vérifier |
| `cargo run --release -- domains/tunnel.dom` | faire tourner le calcul (à partir de l'étape 5) |

`goto` **n'écrase jamais** ce que vous avez écrit : il ne remplit que les blocs encore
occupés par un `todo!()`. Si vous décrochez sur une étape, `goto` la suivante et vous
repartez d'un code cohérent. Si vous voulez explicitement la réponse d'une étape,
`solve <n>` — et `reset <n>` si vous changez d'avis.

## Où écrire

Chaque trou est encadré ainsi :

```rust
pub fn dot(self, other: Vec2) -> f64 {
    // À FAIRE (étape 0) Produit scalaire de deux vecteurs du plan
    // >>> ÉTAPE 0 — à compléter
    todo!("étape 0 — voir le commentaire ci-dessus")
    // <<< ÉTAPE 0
}
```

Écrivez entre les deux marqueurs `>>>` et `<<<`, et laissez-les en place : c'est ainsi
que `goto`, `solve` et `reset` s'y retrouvent. Le reste du fichier vous appartient.

## Prérequis

- Rust ≥ 1.90 (`rustup update stable`)
- rien d'autre : une seule dépendance, `image`, pour encoder les PNG

Vous pouvez versionner votre travail — `git init && git add -A && git commit` — pour
retrouver vos états successifs.

## Licence

CC BY-NC-SA 4.0 — Pascal Havé (contact@haveneer.com).
