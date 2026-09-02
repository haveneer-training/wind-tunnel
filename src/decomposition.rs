//! Découpage du domaine en bandes verticales, une par rang MPI.
//!
//! C'est la partie de l'étape 11 qui ne parle pas de MPI : qui possède quoi, qui envoie
//! quoi à qui, et dans quel ordre. Elle vit dans la bibliothèque parce qu'elle se teste
//! sans MPI installé — le crate `mpi/` n'a plus qu'à faire circuler les octets.
//!
//! Le principe : chaque rang ne construit un maillage que sur **sa** bande, élargie de
//! `halo` colonnes de chaque côté. Personne ne détient le domaine complet — ce serait un
//! contresens vis-à-vis du calcul distribué.
//!
//! ```text
//! masque global, 30 colonnes, 3 bandes, halo de 3 colonnes
//!
//!  colonnes   0 ........... 9 | 10 ......... 19 | 20 ......... 29
//!             \____ rang 0 ___/ \____ rang 1 ___/ \____ rang 2 ___/
//!
//! le rang 1 maille les colonnes 7 à 22 :
//!
//!      7 8 9 | 10 ......... 19 | 20 21 22
//!      \_____/                  \________/
//!       reçues du rang 0         reçues du rang 2
//!            10 11 12 ... 17 18 19
//!            \______/     \______/
//!         envoyées au 0   envoyées au 2
//! ```
//!
//! Les cellules fantômes sont recalculées comme les autres par le solveur, mais leur
//! valeur est écrasée par l'échange avant chaque évaluation de résidu : ce que le voisin
//! sait d'elles fait toujours autorité.

use std::ops::Range;

use crate::error::MeshError;
use crate::field::Field;
use crate::geom::Vec2;
use crate::mask::Mask;
use crate::mesh::{CellId, Mesh};

/// Largeur de halo nécessaire à ce maillage, en colonnes.
///
/// Trois, et le compte se fait couche par couche :
///
/// 1. le résidu d'une cellule possédée lit la **valeur** de sa voisine — 1re couche ;
/// 2. en MUSCL, il lit aussi le **gradient** de cette voisine, qui est calculé à partir
///    des valeurs de la couche suivante — 2e couche ;
/// 3. ce gradient est un moindres carrés sur les *centroïdes* de la 2e couche, or le
///    centroïde d'une cellule dépend de son chanfrein, et son chanfrein dépend de ses
///    propres voisines — 3e couche.
///
/// Le troisième point est propre à ce maillage : le maillage lui-même a un stencil. À
/// deux colonnes, le calcul décomposé s'écarte du calcul monolithique de près de 10⁻²
/// sur `domains/tunnel.dom` à trois rangs — visible, et pourtant sans rien de faux dans
/// la communication. Voir `docs/etapes/etape-11.md`.
pub const HALO: usize = 3;

/// Le découpage d'un masque en bandes verticales.
#[derive(Clone, Debug)]
pub struct Bands {
    cols: usize,
    parts: usize,
    halo: usize,
}

impl Bands {
    /// Découpe `cols` colonnes en `parts` bandes, avec un halo de `halo` colonnes.
    ///
    /// Échoue si une bande serait plus étroite que le halo : elle ne pourrait pas
    /// fournir à son voisin les colonnes qu'il attend.
    pub fn new(cols: usize, parts: usize, halo: usize) -> Result<Bands, MeshError> {
        if cols == 0 || parts == 0 {
            return Err(MeshError::EmptyDomain);
        }
        if parts > 1 && cols / parts < halo.max(1) {
            return Err(MeshError::BandsTooNarrow { cols, parts, halo });
        }
        Ok(Bands { cols, parts, halo })
    }

    /// Nombre de colonnes du masque découpé.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// Nombre de bandes, c'est-à-dire de rangs.
    pub fn parts(&self) -> usize {
        self.parts
    }

    /// Largeur du halo, en colonnes — voir [`HALO`].
    pub fn halo(&self) -> usize {
        self.halo
    }

    /// Colonnes possédées par la bande `part`, halo exclu.
    ///
    /// Les bandes sont aussi égales que possible ; le reste de la division est réparti
    /// une colonne par bande sur les premières.
    ///
    /// ```
    /// use wind_tunnel::decomposition::Bands;
    /// let b = Bands::new(10, 3, 2).unwrap();
    /// assert_eq!(b.owned(0), 0..4);
    /// assert_eq!(b.owned(1), 4..7);
    /// assert_eq!(b.owned(2), 7..10);
    /// ```
    ///
    /// # Panics
    ///
    /// Si `part` n'est pas un rang de ce découpage.
    pub fn owned(&self, part: usize) -> Range<usize> {
        assert!(part < self.parts, "rang {part} hors du découpage");
        // TODO-STEP:11 Répartir `self.cols` colonnes en `self.parts` bandes aussi égales
        // que possible : chaque bande reçoit `cols / parts` colonnes, et les
        // `cols % parts` premières en reçoivent une de plus. Renvoyer l'intervalle des
        // colonnes de la bande `part`.
        // SOLUTION-BEGIN
        let width = self.cols / self.parts;
        let remainder = self.cols % self.parts;
        let start = part * width + part.min(remainder);
        let end = start + width + usize::from(part < remainder);
        start..end
        // SOLUTION-END
    }

    /// Colonnes effectivement maillées par la bande `part` : les siennes et son halo,
    /// tronqué aux bords du domaine.
    pub fn extended(&self, part: usize) -> Range<usize> {
        let owned = self.owned(part);
        let start = owned.start.saturating_sub(self.halo);
        let end = (owned.end + self.halo).min(self.cols);
        start..end
    }
}

/// Ce qu'un rang doit savoir de sa bande, en numérotation **locale**.
///
/// Les identifiants sont ceux du maillage local, celui que renvoie [`Layout::build_mesh`] :
/// il n'existe nulle part de numérotation globale des cellules, et c'est voulu.
#[derive(Clone, Debug)]
pub struct Layout {
    /// Rang de cette bande.
    pub part: usize,
    /// Nombre total de bandes.
    pub parts: usize,
    /// Colonnes du masque global effectivement maillées.
    pub extended: Range<usize>,
    /// Cellules dont ce rang est responsable — les seules dont la valeur fait autorité.
    pub owned: Vec<CellId>,
    /// Cellules à envoyer au rang de gauche.
    pub send_left: Vec<CellId>,
    /// Cellules fantômes à recevoir du rang de gauche.
    pub recv_left: Vec<CellId>,
    /// Cellules à envoyer au rang de droite.
    pub send_right: Vec<CellId>,
    /// Cellules fantômes à recevoir du rang de droite.
    pub recv_right: Vec<CellId>,
}

impl Layout {
    /// Construit le plan de la bande `part` à partir du masque global.
    ///
    /// L'ordre des listes est celui du parcours ligne par ligne, colonne par colonne, du
    /// masque **global**. Les deux rangs d'une coupure le dérivent donc du même objet et
    /// tombent nécessairement d'accord : c'est ce qui permet à l'échange de n'envoyer
    /// que des valeurs, jamais d'identifiants.
    ///
    /// La règle de tri : une cellule à gauche des colonnes possédées est fantôme et
    /// viendra du rang de gauche ; à droite, du rang de droite ; sinon elle est
    /// possédée, et elle est de plus à *envoyer* si elle tombe dans les [`Bands::halo`]
    /// premières colonnes possédées (pour le voisin de gauche) ou dans les dernières
    /// (pour celui de droite). Un rang de bord n'a pas de voisin de ce côté : ses
    /// listes y restent vides.
    pub fn new(mask: &Mask, bands: &Bands, part: usize) -> Layout {
        // TODO-STEP:11 Parcourir les cellules fluides de la bande étendue ligne par
        // ligne puis colonne par colonne — l'ordre même dans lequel `Mesh::from_mask`
        // numérote les cellules — en tenant un compteur d'identifiant local, et ranger
        // chaque cellule dans les listes ci-dessus d'après sa colonne globale.
        // SOLUTION-BEGIN
        let extended = bands.extended(part);
        let owned_cols = bands.owned(part);
        let halo = bands.halo();

        let mut owned = Vec::new();
        let (mut send_left, mut recv_left) = (Vec::new(), Vec::new());
        let (mut send_right, mut recv_right) = (Vec::new(), Vec::new());
        let mut next = 0u32;

        for row in 0..mask.rows() {
            for col in extended.clone() {
                if !mask.is_fluid(row, col) {
                    continue;
                }
                let id = CellId(next);
                next += 1;

                if col < owned_cols.start {
                    recv_left.push(id);
                } else if col >= owned_cols.end {
                    recv_right.push(id);
                } else {
                    owned.push(id);
                    if part > 0 && col < owned_cols.start + halo {
                        send_left.push(id);
                    }
                    if part + 1 < bands.parts() && col + halo >= owned_cols.end {
                        send_right.push(id);
                    }
                }
            }
        }

        Layout {
            part,
            parts: bands.parts(),
            extended,
            owned,
            send_left,
            recv_left,
            send_right,
            recv_right,
        }
        // SOLUTION-END
    }

    /// Rang voisin de gauche, s'il y en a un.
    pub fn left(&self) -> Option<usize> {
        (self.part > 0).then(|| self.part - 1)
    }

    /// Rang voisin de droite, s'il y en a un.
    pub fn right(&self) -> Option<usize> {
        (self.part + 1 < self.parts).then(|| self.part + 1)
    }

    /// Maillage de la bande, replacé à sa position dans le domaine.
    ///
    /// [`Mesh::from_mask`] place toujours la première colonne en `x = 0` ; la
    /// translation remet la bande où elle est réellement, pour que l'écoulement
    /// analytique — évalué en coordonnées globales — et les sorties soient justes.
    pub fn build_mesh(&self, mask: &Mask, h: f64) -> Result<Mesh, MeshError> {
        let mut mesh = Mesh::from_mask(&mask.columns(self.extended.clone()), h)?;
        mesh.translate(Vec2::new(self.extended.start as f64 * h, 0.0));
        Ok(mesh)
    }

    /// Identifiant, dans un maillage du domaine entier, de chaque cellule locale.
    ///
    /// Sert aux tests et à l'assemblage d'une sortie unique. Le calcul distribué, lui,
    /// n'en a jamais besoin : c'est bien le signe qu'aucun rang n'a à connaître le
    /// domaine complet.
    pub fn global_cells(&self, mask: &Mask) -> Vec<CellId> {
        let mut ids = Vec::with_capacity(self.owned.len());
        let mut next = 0u32;
        for row in 0..mask.rows() {
            for col in 0..mask.cols() {
                if !mask.is_fluid(row, col) {
                    continue;
                }
                if self.extended.contains(&col) {
                    ids.push(CellId(next));
                }
                next += 1;
            }
        }
        ids
    }
}

/// Masse de traceur portée par les cellules **possédées** : `Σ cᵢ · |Ωᵢ|`.
///
/// C'est [`Field::total_mass`](crate::field::Field::total_mass) restreint à ce qui
/// appartient au rang. Sommer le champ entier compterait chaque cellule fantôme une
/// seconde fois, chez son propriétaire *et* chez son emprunteur : le diagnostic
/// dépendrait alors du nombre de rangs, ce qui est le meilleur moyen de croire à un
/// défaut de conservation qui n'existe pas.
pub fn owned_mass(mesh: &Mesh, field: &Field, layout: &Layout) -> f64 {
    layout
        .owned
        .iter()
        .map(|&id| field[id] * mesh.cell(id).area)
        .sum()
}

/// Extrema du champ sur les cellules possédées.
pub fn owned_min_max(field: &Field, layout: &Layout) -> (f64, f64) {
    layout
        .owned
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &id| {
            (lo.min(field[id]), hi.max(field[id]))
        })
}

/// Recopie dans un tampon les valeurs des cellules désignées.
///
/// Alloue son tampon : pratique dans un test, à éviter dans une boucle en temps, où
/// [`pack_into`] réutilise le même d'un pas à l'autre.
pub fn pack(field: &Field, ids: &[CellId]) -> Vec<f64> {
    ids.iter().map(|&id| field[id]).collect()
}

/// Recopie les valeurs des cellules désignées dans un tampon déjà alloué.
///
/// C'est [`pack`] sans l'allocation : le tampon est vidé puis rempli, donc sa capacité
/// — acquise au premier appel — sert à tous les suivants.
pub fn pack_into(field: &Field, ids: &[CellId], out: &mut Vec<f64>) {
    out.clear();
    out.extend(ids.iter().map(|&id| field[id]));
}

/// Replace dans le champ les valeurs reçues, dans l'ordre des cellules désignées.
///
/// # Panics
///
/// Si le tampon n'a pas la longueur attendue — le symptôme d'un désaccord entre les
/// deux côtés d'une coupure.
pub fn unpack(field: &mut Field, ids: &[CellId], values: &[f64]) {
    assert_eq!(
        ids.len(),
        values.len(),
        "le voisin a envoyé {} valeurs pour {} cellules fantômes",
        values.len(),
        ids.len()
    );
    for (&id, &value) in ids.iter().zip(values) {
        field[id] = value;
    }
}
