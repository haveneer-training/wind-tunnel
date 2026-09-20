//! Erreurs du projet.
//!
//! Deux types suffisent : ce qui peut mal se passer en construisant le maillage,
//! et ce qui peut mal se passer en le faisant tourner. Chaque variante porte de quoi
//! diagnostiquer sans relire le code — un numéro de ligne, une cellule, une valeur.
//!
//! `Display`, `Error` et `From` sont écrits à la main : les crates `thiserror` et
//! `anyhow` font cela pour vous en production, mais il faut avoir vu ce qu'elles
//! engendrent au moins une fois.

use std::fmt;
use std::io;

use crate::geom::Point;
use crate::mesh::CellId;

/// Ce qui peut échouer entre le fichier de masque et le maillage construit.
#[derive(Debug)]
pub enum MeshError {
    /// Le fichier n'a pas pu être lu.
    Io(io::Error),
    /// Le masque ne contient aucune cellule fluide.
    EmptyDomain,
    /// Deux lignes du masque n'ont pas la même longueur.
    RaggedMask {
        /// Numéro de ligne (à partir de 1) dans le fichier.
        line: usize,
        /// Largeur attendue, fixée par la première ligne de contenu.
        expected: usize,
        /// Largeur effectivement lue.
        got: usize,
    },
    /// Le masque contient un caractère qui n'est ni fluide ni solide.
    InvalidChar {
        /// Numéro de ligne (à partir de 1) dans le fichier.
        line: usize,
        /// Numéro de colonne (à partir de 1).
        col: usize,
        /// Le caractère fautif.
        ch: char,
    },
    /// Le domaine fluide est en plusieurs morceaux : le solveur n'aurait aucun sens.
    Disconnected {
        /// Nombre de composantes connexes trouvées.
        components: usize,
    },
    /// Une cellule d'aire nulle ou négative a été engendrée.
    DegenerateCell {
        /// La cellule fautive.
        cell: CellId,
        /// Son aire signée.
        area: f64,
    },
    /// Le découpage en bandes demandé donnerait des bandes plus étroites que le halo.
    ///
    /// Propre à l'étape 11 : une bande qui ne compte pas au moins `halo` colonnes ne
    /// peut pas fournir à son voisin les colonnes qu'il attend.
    BandsTooNarrow {
        /// Nombre de colonnes du masque.
        cols: usize,
        /// Nombre de bandes demandé.
        parts: usize,
        /// Largeur du halo, en colonnes.
        halo: usize,
    },
    /// Une arête est partagée par plus de deux cellules : le maillage n'est pas une surface.
    NonManifoldEdge {
        /// Premier sommet de l'arête.
        a: u32,
        /// Second sommet de l'arête.
        b: u32,
    },
    /// Un obstacle touche le bord du domaine : le sommet partagé ne peut pas porter à
    /// la fois la condition de bord et celle de l'obstacle (voir `Mesh::classify_boundaries`).
    ObstacleTouchesBoundary {
        /// Coordonnées du sommet partagé.
        at: Point,
    },
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshError::Io(e) => write!(f, "lecture du masque impossible : {e}"),
            MeshError::EmptyDomain => write!(f, "le masque ne contient aucune cellule fluide"),
            MeshError::RaggedMask {
                line,
                expected,
                got,
            } => write!(
                f,
                "ligne {line} : largeur {got}, alors que la première ligne en annonce {expected}"
            ),
            MeshError::InvalidChar { line, col, ch } => write!(
                f,
                "ligne {line}, colonne {col} : caractère {ch:?} inattendu (attendu « . » ou « # »)"
            ),
            MeshError::BandsTooNarrow { cols, parts, halo } => write!(
                f,
                "{cols} colonnes en {parts} bandes donnent des bandes de moins de {halo} \
                 colonne(s) : réduisez le nombre de rangs, ou raffinez le masque"
            ),
            MeshError::Disconnected { components } => write!(
                f,
                "le domaine fluide compte {components} morceaux séparés ; il en faut exactement un"
            ),
            MeshError::DegenerateCell { cell, area } => write!(
                f,
                "la cellule {} a une aire dégénérée ({area:e})",
                cell.index()
            ),
            MeshError::NonManifoldEdge { a, b } => write!(
                f,
                "l'arête ({a}, {b}) est partagée par plus de deux cellules"
            ),
            MeshError::ObstacleTouchesBoundary { at } => write!(
                f,
                "l'obstacle touche le bord du domaine en ({:.3}, {:.3}) : \
                 laissez un jeu d'au moins une cellule entre l'obstacle et le bord",
                at.x, at.y
            ),
        }
    }
}

impl std::error::Error for MeshError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MeshError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for MeshError {
    fn from(e: io::Error) -> Self {
        MeshError::Io(e)
    }
}

/// Ce qui peut échouer pendant le calcul.
#[derive(Debug)]
pub enum SolverError {
    /// Le pas de temps demandé viole la condition de stabilité (CFL).
    ///
    /// Détecté *avant* de lancer le calcul : une intégration explicite instable ne
    /// produit pas un résultat approximatif, elle produit du bruit puis des `NaN`.
    Cfl {
        /// Pas de temps demandé.
        dt: f64,
        /// Pas de temps maximal admissible.
        dt_max: f64,
    },
    /// Rien ne peut bouger : aucune cellule n'a de débit sortant ni de diffusion.
    ///
    /// Le cas se produit quand l'écoulement porteur est nul *et* la diffusivité nulle.
    /// La condition CFL n'impose alors aucune borne — le pas de temps « maximal stable »
    /// serait infini — et le calcul demandé n'a pas de sens : le champ initial est déjà
    /// la solution, à tous les temps.
    NoTransport,
    /// Une valeur non finie est apparue dans le champ.
    NotFinite {
        /// Numéro du pas de temps fautif.
        step: usize,
        /// Première cellule où la valeur n'est plus finie.
        cell: CellId,
    },
    /// Un algorithme itératif n'a pas convergé dans le budget imparti.
    NotConverged {
        /// Nombre d'itérations effectuées.
        iters: usize,
        /// Résidu atteint.
        residual: f64,
    },
    /// L'écriture d'un résultat a échoué.
    Output(io::Error),
    /// Le maillage lui-même est en cause.
    Mesh(MeshError),
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverError::Cfl { dt, dt_max } => write!(
                f,
                "pas de temps {dt:e} instable : la condition CFL impose au plus {dt_max:e}"
            ),
            SolverError::NoTransport => write!(
                f,
                "ni convection ni diffusion : l'écoulement est nul et la diffusivité \
                 aussi, aucun pas de temps n'est plus contraignant qu'un autre"
            ),
            SolverError::NotFinite { step, cell } => write!(
                f,
                "valeur non finie au pas {step}, cellule {} : le calcul a divergé",
                cell.index()
            ),
            SolverError::NotConverged { iters, residual } => write!(
                f,
                "pas de convergence après {iters} itérations (résidu {residual:e})"
            ),
            SolverError::Output(e) => write!(f, "écriture du résultat impossible : {e}"),
            SolverError::Mesh(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SolverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SolverError::Mesh(e) => Some(e),
            SolverError::Output(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for SolverError {
    fn from(e: io::Error) -> Self {
        SolverError::Output(e)
    }
}

impl From<MeshError> for SolverError {
    fn from(e: MeshError) -> Self {
        SolverError::Mesh(e)
    }
}
