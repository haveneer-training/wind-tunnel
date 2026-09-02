//! Un champ scalaire discret : une valeur par cellule.
//!
//! Le stockage est un unique `Vec<f64>` — une « structure de tableaux » plutôt qu'un
//! « tableau de structures ». Les valeurs voisines dans le calcul sont voisines en
//! mémoire, et la boucle en temps ne fait aucune allocation : elle réutilise des
//! tampons alloués une fois pour toutes.

use std::ops::{Index, IndexMut};

use crate::mesh::{CellId, Mesh};

/// Un champ scalaire aux cellules.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    data: Vec<f64>,
}

impl Field {
    /// Champ nul de `n` cellules.
    pub fn zeros(n: usize) -> Field {
        Field { data: vec![0.0; n] }
    }

    /// Champ constant de `n` cellules.
    pub fn filled(n: usize, value: f64) -> Field {
        Field {
            data: vec![value; n],
        }
    }

    /// Construit un champ en évaluant une fonction sur chaque cellule.
    pub fn from_fn(mesh: &Mesh, mut f: impl FnMut(CellId) -> f64) -> Field {
        Field {
            data: (0..mesh.n_cells()).map(|i| f(CellId(i as u32))).collect(),
        }
    }

    /// Nombre de cellules.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Le champ est-il vide ?
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Vue en lecture sur les valeurs.
    pub fn as_slice(&self) -> &[f64] {
        &self.data
    }

    /// Vue en écriture sur les valeurs.
    pub fn as_mut_slice(&mut self) -> &mut [f64] {
        &mut self.data
    }

    /// Itère sur les valeurs.
    pub fn iter(&self) -> std::slice::Iter<'_, f64> {
        self.data.iter()
    }

    /// Recopie les valeurs d'un autre champ, sans allouer.
    pub fn copy_from(&mut self, other: &Field) {
        self.data.copy_from_slice(&other.data);
    }

    /// Valeurs extrêmes du champ.
    pub fn min_max(&self) -> (f64, f64) {
        self.data
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)))
    }

    /// Première cellule portant une valeur non finie, s'il y en a une.
    pub fn first_non_finite(&self) -> Option<CellId> {
        self.data
            .iter()
            .position(|v| !v.is_finite())
            .map(|i| CellId(i as u32))
    }

    /// Intégrale du champ sur le domaine : `Σ cᵢ · |Ωᵢ|`.
    ///
    /// C'est la « masse » de traceur. Sur un domaine fermé elle doit rester constante,
    /// ce qui donne un test de non-régression autrement plus sévère qu'une capture
    /// d'écran.
    pub fn total_mass(&self, mesh: &Mesh) -> f64 {
        self.data
            .iter()
            .zip(mesh.cells())
            .map(|(c, cell)| c * cell.area)
            .sum()
    }
}

impl Index<CellId> for Field {
    type Output = f64;
    fn index(&self, id: CellId) -> &f64 {
        &self.data[id.index()]
    }
}

impl IndexMut<CellId> for Field {
    fn index_mut(&mut self, id: CellId) -> &mut f64 {
        &mut self.data[id.index()]
    }
}
