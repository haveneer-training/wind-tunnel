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
                let neighbours = [
                    (row.wrapping_sub(1), col),
                    (row + 1, col),
                    (row, col.wrapping_sub(1)),
                    (row, col + 1),
                ];
                for (r, c) in neighbours {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masque_minimal() {
        let m = Mask::parse("..\n.#\n").unwrap();
        assert_eq!((m.rows(), m.cols()), (2, 2));
        assert_eq!(m.fluid_count(), 3);
        assert!(m.is_fluid(0, 0));
        assert!(!m.is_fluid(1, 1));
    }

    #[test]
    fn commentaires_et_lignes_vides_ignores() {
        let m = Mask::parse("% titre\n\n..\n..\n").unwrap();
        assert_eq!((m.rows(), m.cols()), (2, 2));
    }

    #[test]
    fn hors_grille_n_est_pas_un_obstacle() {
        let m = Mask::parse(".#\n..\n").unwrap();
        assert!(!m.is_obstacle(-1, 0), "l'extérieur est le bord de la veine");
        assert!(!m.is_obstacle(0, 5));
        assert!(m.is_obstacle(0, 1));
        assert!(!m.is_obstacle(0, 0));
    }
}
