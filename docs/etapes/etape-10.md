# Étape 10 — Écriture recouverte : `thread::scope`, `mpsc`, `Mutex`

**Fichier :** `src/io/png.rs` · **Vérification :** `cargo xtask goto 10`, `cargo test`
· **≈ 30 min**

L'étape 9 a paralléliné trois boucles sans rien changer à leur forme : chaque cellule
lisait ses voisines et n'écrivait que sa propre case, donc `.iter()` devenait
`.par_iter()` et le compilateur laissait faire. Le rendu PNG est différent : toutes les
cellules dessinent dans **la même image**. Ce n'est plus de l'écriture disjointe, c'est
de l'écriture *recouverte* — et le compilateur ne peut pas prouver que deux cellules
n'écriront jamais le même pixel (leurs boîtes englobantes peuvent se toucher au bord),
donc il refuse tout simplement l'accès concurrent tant qu'il n'est pas protégé. Aucune
restructuration en *gather* ne fait disparaître ce problème : ici, il faut un verrou.

## L'idée

`std::sync::Mutex<T>` protège une valeur : un seul thread à la fois peut la tenir
(`.lock()`), les autres attendent. On l'utilise avec `std::thread::scope`, qui lance des
threads liés à la portée d'un bloc — elle les rejoint tous avant de rendre la main, donc
les threads peuvent *emprunter* `mesh`, `field` et le `Mutex` sans qu'aucun n'en soit
propriétaire (pas d'`Arc` nécessaire, voir plus bas). Chaque thread traite une tranche de
cellules et signale, via `std::sync::mpsc`, combien il en a traité : c'est le suivi,
volontairement découplé du verrou — l'un protège une donnée partagée, l'autre fait
circuler un résultat d'un thread à l'autre sans rien partager.

## Le piège à éviter

Verrouiller trop tôt. Si le verrou entoure tout le corps de la boucle — y compris le
calcul du polygone, des coins, du test `contains` par pixel — un seul thread travaille
à la fois : la parallélisation ne sert plus à rien, elle ajoute même le coût du verrou.
Calculez d'abord, sans toucher à l'image ; ne prenez le verrou que pour la section qui y
écrit réellement, et le temps le plus court possible.

## Socle

| Bloc | Fichier | Ce qu'il doit devenir |
|---|---|---|
| la boucle de `write_png` | `png.rs` | `mesh.cells()` réparti en tranches (`.chunks(...)`), un `thread::scope` avec un thread par tranche, un `Mutex<RgbImage>` verrouillé le temps de recopier les pixels déjà calculés, et un `mpsc::channel` cloné par thread pour annoncer combien de cellules il a traitées |

## Pourquoi pas `Arc<Mutex<...>>` ?

Le titre du cours parle des deux ensemble, et pourtant ce socle n'utilise que `Mutex`.
`Arc` ne sert qu'à partager la *propriété* d'une valeur entre threads qui vivent
indépendamment de qui les a lancés — le cas de `std::thread::spawn`, dont la fermeture
doit être `'static` : elle pourrait survivre à la fonction qui l'a créée, donc elle ne
peut pas se contenter d'un emprunt. `std::thread::scope` n'a pas ce problème : le
compilateur sait que le scope se termine avant que `write_png` ne rende la main, donc
emprunter suffit. Retenez la règle plutôt que le couple : `Mutex` protège une donnée
partagée, `Arc` ne devient nécessaire que si sa propriété doit l'être aussi.

## Ce qu'il y a à remarquer

```shell
cargo test rendering_is_deterministic_across_runs
cargo test rendering_a_handful_of_cells_still_works
```

Ces tests (`tests/render.rs`) rendent deux fois le même champ et comparent les fichiers
PNG produits **octet pour octet** : rien ne doit dépendre de l'ordre dans lequel les
threads obtiennent le verrou. Le second force un maillage plus petit que le nombre de
threads disponibles — certaines tranches sont vides, ce que le suivi doit encaisser sans
bloquer ni paniquer.

Le `debug_assert_eq!` en fin de fonction (actif en `cargo test`, absent en `--release`)
vérifie que la somme reçue sur le canal `mpsc` égale bien `mesh.n_cells()` : ni cellule
oubliée, ni comptée deux fois. C'est un usage courant du *message passing* — confirmer
qu'un travail réparti a bien été fait en entier — indépendant du verrou qui, lui, protège
l'écriture des pixels.

Chronométrez le rendu sur un maillage fin (`--refine 10` ou plus) : le gain est réel
mais nettement moins net qu'à l'étape 9, parce que le verrou sérialise l'écriture — seul
le calcul (le test `contains`, le plus coûteux de la fonction) profite pleinement des
threads.

## Pour aller plus loin

- Retrouvez `Arc<Mutex<...>>` en vrai : réécrivez ce rendu avec `std::thread::spawn`
  plutôt que `thread::scope`. Le compilateur refusera d'emprunter `mesh`, `field` et
  l'image — il faudra les envelopper dans des `Arc`, un `Arc::clone` par thread, puis
  récupérer l'image via `Arc::try_unwrap` après les `.join()`. Comparez la verbosité à
  la version à `scope` du socle : c'est le prix d'une propriété réellement partagée
  entre threads indépendants, pour un gain nul ici puisque aucun thread ne survit à
  `write_png`.
- Peut-on se passer complètement du verrou ? Faites calculer à chaque thread, plutôt que
  d'écrire dans l'image, la liste de ses pixels (`Vec<(u32, u32, Rgb<u8>)>`), renvoyée
  via le canal `mpsc` à la place du simple compteur ; une dernière boucle séquentielle,
  après le `thread::scope`, recopie le tout dans l'image. Chronométrez face à la version
  à verrou : lequel gagne, et à partir de quelle taille de maillage ?
- Pourquoi ce fichier utilise-t-il des threads « à la main » plutôt que `rayon`, comme à
  l'étape 9 ? Essayez de récrire le socle avec `rayon` et un `Mutex` — ça marche aussi,
  et c'est même plus court. La version à la main sert ici à voir ce que `rayon` cache :
  la répartition du travail en tranches, la portée des threads, le passage de message.
