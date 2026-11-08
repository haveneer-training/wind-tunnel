//! Lecture et validation du masque de domaine.
//!
//! Un masque est un fichier texte : `.` pour une cellule de fluide, `#` pour du solide.
//! Les lignes commençant par `%` sont des commentaires, les lignes vides sont ignorées.
//!
//! ```text
//! % une veine avec un obstacle
//! ...........
//! ....###....
//! ....###....
//! ...........
//! ```
//!
//! Tout l'intérêt du format est qu'il se dessine à la main. Toute sa difficulté est
//! qu'un humain qui dessine à la main se trompe : ligne trop courte, caractère exotique,
//! obstacle qui coupe le domaine en deux. C'est le rôle de ce module de le dire
//! précisément plutôt que de produire un maillage silencieusement faux.

use std::fs;
use std::path::Path;

use crate::error::MeshError;

/// Caractère représentant une cellule de fluide.
pub const FLUID: char = '.';
/// Caractère représentant une cellule solide.
pub const SOLID: char = '#';
/// Préfixe des lignes de commentaire.
pub const COMMENT: char = '%';

/// Une grille rectangulaire de cellules fluides ou solides.
///
/// La ligne 0 est le **haut** du domaine, comme dans le fichier.
#[derive(Clone, Debug)]
pub struct Mask {
    rows: usize,
    cols: usize,
    fluid: Vec<bool>,
}

impl Mask {
    /// Analyse un masque depuis son texte.
    ///
    /// Le `Vec` des cellules est construit ici, rempli ligne par ligne par
    /// [`parse_row`] qui l'emprunte en `&mut`, puis **déplacé** dans le `Mask` rendu à
    /// l'appelant. Personne n'a eu à se demander qui le libère.
    pub fn parse(text: &str) -> Result<Mask, MeshError> {
        let mut fluid = Vec::new();
        let mut cols = 0usize;
        let mut rows = 0usize;

        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.trim_end();
            if line.is_empty() || line.starts_with(COMMENT) {
                continue;
            }
            // La première ligne fixe la largeur ; les suivantes doivent s'y tenir.
            let expected = (rows > 0).then_some(cols);
            cols = parse_row(line, line_no + 1, expected, &mut fluid)?;
            rows += 1;
        }

        if rows == 0 || !fluid.iter().any(|&f| f) {
            return Err(MeshError::EmptyDomain);
        }

        let mask = Mask { rows, cols, fluid };
        mask.check_connected()?;
        Ok(mask)
    }

    /// Lit un masque depuis un fichier.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Mask, MeshError> {
        // `?` convertit l'erreur d'E/S en `MeshError` grâce à `impl From<io::Error>`.
        let text = fs::read_to_string(path)?;
        Mask::parse(&text)
    }

    /// Nombre de lignes.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Nombre de colonnes.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// La cellule `(row, col)` est-elle du fluide ? Hors grille, la réponse est `false`.
    #[cfg_attr(not(feature = "step1"), allow(unused_variables))] // trou étape 1
    pub fn is_fluid(&self, row: usize, col: usize) -> bool {
        // TODO-STEP:1 Répondre `false` hors de la grille, sinon lire la case du tableau
        // stocké ligne par ligne
        // SOLUTION-BEGIN
        if row >= self.rows || col >= self.cols {
            return false;
        }
        self.fluid[row * self.cols + col]
        // SOLUTION-END
    }

    /// La cellule `(row, col)` est-elle une paroi d'obstacle ?
    ///
    /// L'extérieur de la grille ne compte **pas** comme un obstacle : c'est le bord de
    /// la veine, pas un objet placé dedans. La distinction décide du chanfreinage des
    /// coins lors de la construction du maillage.
    ///
    /// Signé en `isize` pour interroger les voisins d'une cellule de bord sans jongler
    /// avec les débordements d'entiers non signés.
    pub fn is_obstacle(&self, row: isize, col: isize) -> bool {
        if row < 0 || col < 0 || row as usize >= self.rows || col as usize >= self.cols {
            return false;
        }
        !self.is_fluid(row as usize, col as usize)
    }

    /// Subdivise chaque case en `factor × factor`, sans rien redessiner.
    ///
    /// Le domaine reste le même, seule la finesse du maillage augmente : c'est le seul
    /// moyen de raffiner. À ne pas confondre avec le pas `h` passé à
    /// [`Mesh::from_mask`](crate::mesh::Mesh::from_mask), qui fixe la *taille* d'une
    /// cellule : le diviser par deux ne change pas le nombre de cellules, cela rétrécit
    /// le domaine d'autant et donne exactement la même image.
    ///
    /// ```
    /// use wind_tunnel::mask::Mask;
    /// let gros = Mask::parse("..\n.#\n").unwrap();
    /// let fin = gros.refine(3);
    /// assert_eq!((fin.rows(), fin.cols()), (6, 6));
    /// assert_eq!(fin.fluid_count(), 9 * gros.fluid_count());
    /// ```
    #[cfg_attr(not(feature = "step1"), allow(unused_variables))] // trou étape 1
    pub fn refine(&self, factor: usize) -> Mask {
        // TODO-STEP:1 Construire le masque subdivisé : `factor × factor` cases à la
        // place de chaque case, chacune reprenant la valeur de celle dont elle vient.
        // Un `factor` de 0 ou 1 ne change rien, et le masque courant est seulement
        // *emprunté* : il faut donc en rendre un nouveau.
        // HINT : la case `(row, col)` du masque raffiné reprend la case
        // `(row / factor, col / factor)` de l'original ; `self.clone()` si `factor <= 1`.
        // Réserver la place d'un coup : `Vec::with_capacity(rows * cols)`, puis deux boucles
        // imbriquées et `push` ; finir par `Mask { rows, cols, fluid }` (forme abrégée).
        // SOLUTION-BEGIN
        if factor <= 1 {
            return self.clone();
        }
        let (rows, cols) = (self.rows * factor, self.cols * factor);
        let mut fluid = Vec::with_capacity(rows * cols);
        for row in 0..rows {
            for col in 0..cols {
                fluid.push(self.is_fluid(row / factor, col / factor));
            }
        }
        Mask { rows, cols, fluid }
        // SOLUTION-END
    }

    /// Extrait la tranche de colonnes `range`, sur toute la hauteur du masque.
    ///
    /// C'est le découpage en bandes verticales de l'étape 11 : chaque rang MPI ne
    /// construit un maillage que sur sa tranche. Contrairement à [`Mask::parse`], la
    /// connexité n'est **pas** vérifiée : une bande qui traverse l'obstacle a du fluide
    /// au-dessus et au-dessous sans chemin entre les deux, et c'est légitime.
    ///
    /// ```
    /// use wind_tunnel::mask::Mask;
    /// let m = Mask::parse("....\n.##.\n....\n").unwrap();
    /// let bande = m.columns(1..3);
    /// assert_eq!((bande.rows(), bande.cols()), (3, 2));
    /// assert_eq!(bande.fluid_count(), 4);
    /// ```
    ///
    /// # Panics
    ///
    /// Si `range` sort de la grille ou est vide.
    pub fn columns(&self, range: std::ops::Range<usize>) -> Mask {
        assert!(
            range.start < range.end && range.end <= self.cols,
            "tranche de colonnes {range:?} hors du masque ({} colonnes)",
            self.cols
        );
        let cols = range.len();
        let mut fluid = Vec::with_capacity(self.rows * cols);
        for row in 0..self.rows {
            for col in range.clone() {
                fluid.push(self.fluid[row * self.cols + col]);
            }
        }
        Mask {
            rows: self.rows,
            cols,
            fluid,
        }
    }

    /// Nombre de cellules fluides.
    pub fn fluid_count(&self) -> usize {
        self.fluid.iter().filter(|&&f| f).count()
    }

    /// Vérifie que le fluide est d'un seul tenant.
    ///
    /// On compte les composantes : chaque cellule fluide qu'aucun parcours n'a encore
    /// atteinte en démarre un nouveau (en largeur, voir `explore`). Plus d'une, et le
    /// domaine est coupé — leur nombre va dans le message d'erreur.
    fn check_connected(&self) -> Result<(), MeshError> {
        // TODO-STEP:1 Compter les composantes : chaque cellule fluide encore non visitée
        // démarre une exploration ; renvoyer `Disconnected` si le compte dépasse un.
        // HINT : `seen` vaut `vec![false; self.fluid.len()]`, puis :
        //     composantes = 0
        //     pour chaque indice k fluide et pas encore vu :
        //         composantes += 1 ; explorer depuis k
        //     composantes > 1 → `Err(MeshError::Disconnected { components })`
        // SOLUTION-BEGIN
        let mut seen = vec![false; self.fluid.len()];
        let mut components = 0usize;
        for start in 0..self.fluid.len() {
            if self.fluid[start] && !seen[start] {
                components += 1;
                self.explore(start, &mut seen);
            }
        }
        if components > 1 {
            return Err(MeshError::Disconnected { components });
        }
        Ok(())
        // SOLUTION-END
    }

    /// Indices des voisines fluides à quatre connexités.
    #[cfg_attr(not(feature = "step2"), allow(dead_code))] // appelée depuis l'exploration
    fn fluid_neighbours(&self, k: usize) -> Vec<usize> {
        // TODO-STEP:1 Convertir l'indice à plat en ligne/colonne et collecter les
        // voisines fluides situées dans la grille, sans diagonales.
        // HINT : piège — `row - 1` sur un `usize` nul panique (débordement). Le plus simple :
        // tester avant de soustraire, voisine par voisine, dans un `Vec` que l'on remplit :
        //     si row > 0 et is_fluid(row - 1, col) → pousser (row - 1)·cols + col
        //     si is_fluid(row + 1, col)            → pousser (row + 1)·cols + col
        //     (idem à gauche avec col > 0, et à droite)
        // `is_fluid` répond déjà `false` au-delà du bord bas ou droit. Variante (J2, avec
        // `Option`) : `row.checked_sub(1)` rend `None` au lieu de paniquer, et
        // `filter_map(|(r, c)| Some((r?, c?)))` écarte les candidats hors grille.
        // SOLUTION-BEGIN
        let (row, col) = (k / self.cols, k % self.cols);
        [
            (row.checked_sub(1), Some(col)),
            (row.checked_add(1), Some(col)),
            (Some(row), col.checked_sub(1)),
            (Some(row), col.checked_add(1)),
        ]
        .into_iter()
        .filter_map(|(r, c)| Some((r?, c?)))
        .filter(|&(r, c)| self.is_fluid(r, c))
        .map(|(r, c)| r * self.cols + c)
        .collect()
        // SOLUTION-END
    }

    /// Marque et explore toute la composante contenant `start`.
    #[cfg_attr(not(feature = "step2"), allow(dead_code))] // appelée depuis check_connected
    fn explore(&self, start: usize, seen: &mut [bool]) {
        // TODO-STEP:1 Parcourir la composante avec une file ; marquer chaque cellule
        // avant de l'ajouter afin qu'elle ne soit mise en file qu'une seule fois.
        // HINT : un simple `Vec` sert de pile — `vec![start]`, `push`, `pop` :
        //     marquer start ; pile = [start]
        //     tant que la pile n'est pas vide :
        //         k = pile.pop().unwrap()
        //         pour chaque voisine v non marquée de `self.fluid_neighbours(k)` :
        //             marquer v, l'empiler
        // L'ordre de visite (en profondeur) importe peu : seul compte qui est atteint. Pas
        // de récursion : sur un maillage raffiné, la pile d'appels déborderait. `clippy`
        // (étape 6) vous proposera `while let Some(k) = pile.pop()` à la place du test.
        // Variante : `std::collections::VecDeque` (`push_back`/`pop_front`) donne un
        // parcours en largeur.
        // SOLUTION-BEGIN
        let mut queue = std::collections::VecDeque::from([start]);
        seen[start] = true;
        while let Some(k) = queue.pop_front() {
            for neighbour in self.fluid_neighbours(k) {
                if !seen[neighbour] {
                    seen[neighbour] = true;
                    queue.push_back(neighbour);
                }
            }
        }
        // SOLUTION-END
    }
}

/// Analyse une ligne du masque et empile ses cellules dans `fluid`.
///
/// `expected` vaut `None` pour la première ligne — c'est elle qui fixe la largeur du
/// domaine — et `Some(largeur)` pour toutes les suivantes, qui doivent s'y tenir. La
/// valeur rendue est la largeur de la ligne analysée.
///
/// `line_no` et les colonnes sont comptés **à partir de 1** : ce sont des numéros
/// destinés à un humain qui ouvrira le fichier dans un éditeur, pas des indices.
///
/// Deux définitions, selon la convention du dispositif d'étapes : celle-ci, complète,
/// vaut jusqu'à l'étape 2, sans quoi *aucun* masque ne se lirait et les tests des étapes
/// 1 et 2 paniqueraient tous. L'étape 3 — celle des erreurs typées — la remplace par la
/// version à trou, juste en dessous.
#[cfg(not(feature = "step3"))]
fn parse_row(
    line: &str,
    line_no: usize,
    expected: Option<usize>,
    fluid: &mut Vec<bool>,
) -> Result<usize, MeshError> {
    let width = line.chars().count();
    if let Some(cols) = expected
        && width != cols
    {
        return Err(MeshError::RaggedMask {
            line: line_no,
            expected: cols,
            got: width,
        });
    }

    for (col, ch) in line.chars().enumerate() {
        match ch {
            FLUID => fluid.push(true),
            SOLID => fluid.push(false),
            other => {
                return Err(MeshError::InvalidChar {
                    line: line_no,
                    col: col + 1,
                    ch: other,
                });
            }
        }
    }

    Ok(width)
}

/// Convertit un caractère du masque en indicateur de fluide.
#[cfg(feature = "step3")]
#[cfg_attr(not(feature = "step4"), allow(dead_code))] // appelée depuis le trou de l'étape 3
#[cfg_attr(not(feature = "step3"), allow(unused_variables))] // trou étape 3
fn cell_of(ch: char, line: usize, col: usize) -> Result<bool, MeshError> {
    // TODO-STEP:3 Associer `FLUID` à `true`, `SOLID` à `false` et tout autre caractère
    // à `MeshError::InvalidChar`, avec les coordonnées reçues.
    // HINT : les constantes `FLUID` et `SOLID` servent directement de motifs dans un `match`.
    // Piège : un nom en minuscules dans un motif (`fluid =>`) n'est pas une comparaison mais
    // une nouvelle variable, qui attrape tout. Le dernier bras, `ch => Err(…)`, en profite.
    // SOLUTION-BEGIN
    match ch {
        FLUID => Ok(true),
        SOLID => Ok(false),
        ch => Err(MeshError::InvalidChar { line, col, ch }),
    }
    // SOLUTION-END
}

/// Vérifie que la largeur d'une ligne correspond à celle du masque.
#[cfg(feature = "step3")]
#[cfg_attr(not(feature = "step4"), allow(dead_code))] // appelée depuis le trou de parse_row
fn check_width(width: usize, expected: Option<usize>, line_no: usize) -> Result<(), MeshError> {
    // TODO-STEP:3 Comparer la largeur à la largeur attendue, si elle existe, et
    // construire `RaggedMask` avec les coordonnées fournies.
    // HINT : en édition 2024, `if let Some(expected) = expected && width != expected { … }`.
    // SOLUTION-BEGIN
    if let Some(expected) = expected
        && width != expected
    {
        return Err(MeshError::RaggedMask {
            line: line_no,
            expected,
            got: width,
        });
    }
    Ok(())
    // SOLUTION-END
}

/// Analyse une ligne du masque et empile ses cellules dans `fluid` — version de l'étape
/// 3. Voir la documentation de l'autre définition, juste au-dessus.
#[cfg(feature = "step3")]
fn parse_row(
    line: &str,
    line_no: usize,
    expected: Option<usize>,
    fluid: &mut Vec<bool>,
) -> Result<usize, MeshError> {
    // TODO-STEP:3 Calculer et vérifier la largeur, convertir les caractères avec
    // `cell_of`, puis renvoyer la largeur.
    // HINT : `line.len()` compte des **octets**, pas des caractères : `line.chars().count()`.
    // `?` après `check_width(…)` et `cell_of(…)` remonte leur erreur telle quelle ;
    // `for (col, ch) in line.chars().enumerate()` compte à partir de 0, les colonnes
    // affichées à partir de 1 (`col + 1`).
    // SOLUTION-BEGIN
    let width = line.chars().count();
    check_width(width, expected, line_no)?;
    for (col, ch) in line.chars().enumerate() {
        fluid.push(cell_of(ch, line_no, col + 1)?);
    }
    Ok(width)
    // SOLUTION-END
}

#[cfg(all(test, feature = "step1"))]
mod tests {
    use super::*;

    #[test]
    fn minimal_mask() {
        let m = Mask::parse("..\n.#\n").unwrap();
        assert_eq!((m.rows(), m.cols()), (2, 2));
        assert_eq!(m.fluid_count(), 3);
        assert!(m.is_fluid(0, 0));
        assert!(!m.is_fluid(1, 1));
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let m = Mask::parse("% titre\n\n..\n..\n").unwrap();
        assert_eq!((m.rows(), m.cols()), (2, 2));
    }

    #[test]
    fn outside_the_grid_is_not_an_obstacle() {
        let m = Mask::parse(".#\n..\n").unwrap();
        assert!(!m.is_obstacle(-1, 0), "l'extérieur est le bord de la veine");
        assert!(!m.is_obstacle(0, 5));
        assert!(m.is_obstacle(0, 1));
        assert!(!m.is_obstacle(0, 0));
    }

    #[test]
    fn fluid_neighbours_are_four_connected_and_stay_in_bounds() {
        let m = Mask {
            rows: 2,
            cols: 2,
            fluid: vec![true, false, false, true],
        };
        assert!(m.fluid_neighbours(0).is_empty());
        assert!(m.fluid_neighbours(3).is_empty());
    }

    #[test]
    fn explore_marks_only_the_starting_component() {
        let m = Mask {
            rows: 2,
            cols: 3,
            fluid: vec![true, true, false, false, false, true],
        };
        let mut seen = vec![false; 6];
        m.explore(0, &mut seen);
        assert_eq!(seen, [true, true, false, false, false, false]);
    }

    #[cfg(feature = "step3")]
    #[test]
    fn check_width_accepts_first_row_and_reports_ragged_rows() {
        assert!(check_width(3, None, 1).is_ok());
        assert!(matches!(
            check_width(2, Some(3), 4),
            Err(MeshError::RaggedMask {
                line: 4,
                expected: 3,
                got: 2,
            })
        ));
    }

    #[cfg(feature = "step3")]
    #[test]
    fn cell_of_reports_one_based_coordinates() {
        assert!(cell_of(FLUID, 4, 7).unwrap());
        assert!(!cell_of(SOLID, 4, 7).unwrap());
        assert!(matches!(
            cell_of('x', 4, 7),
            Err(MeshError::InvalidChar {
                line: 4,
                col: 7,
                ch: 'x'
            })
        ));
    }
}
