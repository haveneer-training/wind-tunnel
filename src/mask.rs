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
    pub fn parse(text: &str) -> Result<Mask, MeshError> {
        let mut fluid = Vec::new();
        let mut cols = 0usize;
        let mut rows = 0usize;

        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.trim_end();
            if line.is_empty() || line.starts_with(COMMENT) {
                continue;
            }
            let width = line.chars().count();
            if rows == 0 {
                cols = width;
            } else if width != cols {
                return Err(MeshError::RaggedMask {
                    line: line_no + 1,
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
                            line: line_no + 1,
                            col: col + 1,
                            ch: other,
                        })
                    }
                }
            }
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
    pub fn refine(&self, factor: usize) -> Mask {
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
    /// Un parcours en largeur suffit : on part de la première cellule fluide, et si
    /// l'on n'a pas tout atteint, on compte les morceaux restants pour le message.
    fn check_connected(&self) -> Result<(), MeshError> {
        // TODO-STEP:1 (pour aller plus loin) Compter les composantes connexes du fluide
        // par parcours en largeur, et signaler `Disconnected` s'il y en a plus d'une
        // SOLUTION-BEGIN
        let mut seen = vec![false; self.fluid.len()];
        let mut components = 0usize;
        let mut queue = Vec::new();

        for start in 0..self.fluid.len() {
            if !self.fluid[start] || seen[start] {
                continue;
            }
            components += 1;
            seen[start] = true;
            queue.push(start);
            while let Some(k) = queue.pop() {
                let (row, col) = (k / self.cols, k % self.cols);
                let neighbors = [
                    (row.wrapping_sub(1), col),
                    (row + 1, col),
                    (row, col.wrapping_sub(1)),
                    (row, col + 1),
                ];
                for (r, c) in neighbors {
                    if r < self.rows && c < self.cols && self.is_fluid(r, c) {
                        let n = r * self.cols + c;
                        if !seen[n] {
                            seen[n] = true;
                            queue.push(n);
                        }
                    }
                }
            }
        }

        if components > 1 {
            return Err(MeshError::Disconnected { components });
        }
        Ok(())
        // SOLUTION-END
    }
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
}
