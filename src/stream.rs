//! Écoulement **calculé** sur le maillage : la fonction de courant `ψ` y est résolue,
//! au lieu d'être donnée par une formule.
//!
//! L'écoulement analytique de [`crate::velocity::PotentialCylinder`] a deux défauts, et
//! ce sont les mêmes depuis l'étape 4 : il contourne un disque de même aire que
//! l'obstacle plutôt que l'obstacle lui-même, et les parois en escalier de la veine ne
//! sont pas exactement des lignes de courant, si bien qu'un petit débit résiduel les
//! traverse. Le second défaut est ce qui force les parois en `ZeroGradient` plutôt qu'en
//! `NoFlux` (voir `Config::default`).
//!
//! On calcule donc `ψ` là où le solveur la lit — **aux sommets** — en résolvant
//! `∇²ψ = 0`, avec `ψ` imposée sur les bords solides. Deux propriétés en découlent :
//!
//! * `ψ` constante sur les sommets d'une paroi ⇒ le débit de chaque face de cette paroi
//!   vaut `ψ(b) − ψ(a)`, soit exactement `0.0` — pas « petit », *zéro*. Les parois
//!   deviennent des lignes de courant au sens de l'arithmétique flottante, et `NoFlux`
//!   devient légitime.
//! * la forme réellement dessinée est respectée : aucune formule ne vient s'y substituer.
//!
//! Ce qui **ne** dépend pas de ce module : la conservation. Le débit d'une face étant une
//! différence de `ψ`, le bilan d'une cellule fermée est une somme télescopique, nulle à
//! la précision machine *pour n'importe quel* champ `ψ` aux sommets. Un itéré non
//! convergé donne un écoulement moins juste, jamais un schéma non conservatif. La
//! propriété vient de la forme du schéma, pas de la qualité du modèle.
//!
//! # Discrétisation
//!
//! Chaque face interne d'extrémités `(a, b)` est une arête du maillage dual, de poids
//! `w = face.distance / face.length`, et l'équation du sommet `v` est
//!
//! ```text
//! Σ  w (ψ_voisin − ψ_v) = 0
//! ```
//!
//! Sur une grille de carrés de côté `h`, `distance == length == h` donc `w == 1` : on
//! retombe sur le laplacien à cinq points. Les arêtes diagonales des triangles
//! chanfreinés ont `length = h√2` et une distance plus courte, donc un poids plus faible.

use rayon::prelude::*;

use crate::error::SolverError;
use crate::geom::Point;
// `BoundaryKind` ne sert que dans le corps de `boundary_values` : troué, l'import
// devient inutile, et cet avertissement-là n'apprendrait rien au stagiaire.
#[cfg_attr(not(feature = "step13"), allow(unused_imports))] // collatéral des trous étape 12
use crate::mesh::BoundaryKind;
use crate::mesh::{Mesh, VertexId};
use crate::velocity::StreamSource;

/// Réglages de la résolution de `ψ`.
#[derive(Clone, Copy, Debug)]
pub struct StreamOptions {
    /// Vitesse débitante de la veine, qui fixe l'échelle de `ψ`.
    pub speed: f64,
    /// Nombre maximal de balayages avant d'abandonner.
    pub max_iters: usize,
    /// Résidu relatif visé.
    pub tol: f64,
}

impl Default for StreamOptions {
    fn default() -> Self {
        StreamOptions {
            speed: 1.0,
            max_iters: 200_000,
            tol: 1e-6,
        }
    }
}

/// Voisinage pondéré de chaque sommet, au format CSR.
///
/// Même format que la connectivité cellule → faces du maillage : un tableau d'offsets,
/// et les voisins de tous les sommets bout à bout. C'est la matrice du système, sans
/// jamais en stocker une : on ne garde que ce qui est non nul, et le produit
/// matrice-vecteur se lit directement dessus.
#[cfg_attr(not(feature = "step13"), allow(dead_code))] // collatéral des trous étape 12
struct Adjacency {
    offsets: Vec<u32>,
    neighbours: Vec<VertexId>,
    weights: Vec<f64>,
}

impl Adjacency {
    /// Voisins du sommet `id`, et le poids de chaque arête.
    fn of(&self, id: VertexId) -> (&[VertexId], &[f64]) {
        let start = self.offsets[id.index()] as usize;
        let end = self.offsets[id.index() + 1] as usize;
        (&self.neighbours[start..end], &self.weights[start..end])
    }
}

/// Construit le voisinage pondéré des sommets à partir des faces internes.
///
/// Une face interne `(a, b)` donne **deux** arêtes, `a → b` et `b → a`, de même poids
/// `w = distance / length`. Les faces de bord sont ignorées : leurs deux extrémités sont
/// soit imposées (entrée, parois, obstacle), soit libres avec leurs seules arêtes
/// internes pour équation — ce qui est exactement la condition de Neumann homogène qu'on
/// veut à la sortie.
///
/// Comme pour la connectivité du maillage, deux passes : compter les arêtes de chaque
/// sommet pour construire `offsets`, puis remplir en avançant un curseur par sommet.
#[cfg_attr(not(feature = "step12"), allow(unused_variables))] // trou étape 12
fn adjacency(mesh: &Mesh) -> Adjacency {
    // TODO-STEP:12 Construire le CSR décrit ci-dessus : compter, cumuler, remplir.
    // SOLUTION-BEGIN
    let n = mesh.n_vertices();
    let mut counts = vec![0u32; n + 1];
    for face in mesh.faces() {
        if face.is_boundary() {
            continue;
        }
        counts[face.a.index() + 1] += 1;
        counts[face.b.index() + 1] += 1;
    }
    let mut offsets = counts;
    for i in 0..n {
        offsets[i + 1] += offsets[i];
    }

    let total = offsets[n] as usize;
    let mut neighbours = vec![VertexId(0); total];
    let mut weights = vec![0.0; total];
    let mut cursor: Vec<u32> = offsets[..n].to_vec();
    for face in mesh.faces() {
        if face.is_boundary() {
            continue;
        }
        let w = face.distance / face.length;
        for (from, to) in [(face.a, face.b), (face.b, face.a)] {
            let slot = cursor[from.index()] as usize;
            neighbours[slot] = to;
            weights[slot] = w;
            cursor[from.index()] += 1;
        }
    }
    Adjacency {
        offsets,
        neighbours,
        weights,
    }
    // SOLUTION-END
}

/// Valeur imposée à chaque sommet, `None` si elle est libre.
///
/// L'écoulement de référence est horizontal et de débit `speed` par unité de hauteur, de
/// fonction de courant `ψ₀(y) = speed × (y − ymin)`. On impose :
///
/// * `Inlet` : `ψ₀(y)` du sommet — le profil d'entrée ;
/// * `Wall` : `ψ₀(y)` du sommet aussi, ce qui donne `0` en bas et le débit total en haut,
///   les deux étant bien constants le long de leur paroi ;
/// * `Obstacle` : une **constante**, la même pour tous ses sommets — c'est elle qui rend
///   l'obstacle étanche. Sa valeur est indéterminée par le problème (elle répartit le
///   débit entre le dessus et le dessous) ; on prend `ψ₀` de l'ordonnée moyenne de son
///   contour, exacte pour un obstacle symétrique en `y`.
/// * `Outlet` : rien, la sortie est libre.
///
/// L'entrée et les parois se partagent la même formule, donc les coins où elles se
/// rencontrent sont cohérents sans cas particulier.
#[cfg_attr(not(feature = "step12"), allow(unused_variables))] // trou étape 12
fn boundary_values(mesh: &Mesh, options: &StreamOptions) -> Vec<Option<f64>> {
    // TODO-STEP:12 Renvoyer un tableau par sommet, rempli groupe de bord par groupe de
    // bord (`mesh.groups()`), en suivant la liste du commentaire ci-dessus.
    // SOLUTION-BEGIN
    let (_, ymin, _, _) = mesh.bounds();
    let vertices = mesh.vertices();
    let psi0 = |p: Point| options.speed * (p.y - ymin);

    let mut fixed = vec![None; mesh.n_vertices()];
    let mut set = |group: Option<&Vec<crate::mesh::FaceId>>, value: &dyn Fn(Point) -> f64| {
        for id in group.into_iter().flatten() {
            let face = mesh.face(*id);
            for v in [face.a, face.b] {
                fixed[v.index()] = Some(value(vertices[v.index()]));
            }
        }
    };
    let groups = mesh.groups();
    set(groups.get(&BoundaryKind::Inlet), &psi0);
    set(groups.get(&BoundaryKind::Wall), &psi0);

    // L'obstacle n'est pas traité par la même formule : il lui faut *une* constante,
    // sinon ses faces ne sont pas étanches. On la prend au milieu de ses sommets.
    if let Some(faces) = groups.get(&BoundaryKind::Obstacle) {
        let mut sum = 0.0;
        let mut count = 0.0;
        for id in faces {
            let face = mesh.face(*id);
            for v in [face.a, face.b] {
                sum += vertices[v.index()].y;
                count += 1.0;
            }
        }
        let value = psi0(Point::new(0.0, sum / count));
        set(Some(faces), &|_| value);
    }
    fixed
    // SOLUTION-END
}

/// Résidu `max |Σ w (ψ_voisin − ψ_v)|` sur les sommets libres, rapporté à l'échelle de `ψ`.
fn residual_inf(adjacency: &Adjacency, fixed: &[Option<f64>], psi: &[f64], scale: f64) -> f64 {
    let worst = (0..psi.len())
        .into_par_iter()
        .filter(|&i| fixed[i].is_none())
        .map(|i| {
            let (neighbours, weights) = adjacency.of(VertexId(i as u32));
            let sum: f64 = neighbours
                .iter()
                .zip(weights)
                .map(|(n, w)| w * (psi[n.index()] - psi[i]))
                .sum();
            sum.abs()
        })
        .reduce(|| 0.0, f64::max);
    worst / scale
}

/// Balayages de Jacobi jusqu'au résidu visé. Renvoie le nombre d'itérations et le résidu.
///
/// Un balayage remplace la valeur de chaque sommet libre par la moyenne pondérée de ses
/// voisins, `ψ_v ← (Σ w ψ_voisin) / (Σ w)` ; les sommets imposés ne bougent pas. On lit
/// `psi`, on écrit `next`, puis on échange les deux — jamais sur place, sinon le résultat
/// dépend de l'ordre de parcours et la version parallèle ne serait pas reproductible.
#[cfg_attr(not(feature = "step12"), allow(unused_variables))] // trou étape 12
fn jacobi(
    adjacency: &Adjacency,
    fixed: &[Option<f64>],
    psi: &mut Vec<f64>,
    scale: f64,
    options: &StreamOptions,
) -> Result<(usize, f64), SolverError> {
    // TODO-STEP:12 Boucler jusqu'à `options.max_iters` : un balayage écrit dans un second
    // tampon, le résidu par `residual_inf`, et l'arrêt dès qu'il passe sous
    // `options.tol`. Si le budget s'épuise, c'est `SolverError::NotConverged`.
    // SOLUTION-BEGIN
    let mut next = psi.clone();
    for iter in 1..=options.max_iters {
        next.par_iter_mut().enumerate().for_each(|(i, slot)| {
            *slot = match fixed[i] {
                Some(value) => value,
                None => {
                    let (neighbours, weights) = adjacency.of(VertexId(i as u32));
                    let sum: f64 = neighbours
                        .iter()
                        .zip(weights)
                        .map(|(n, w)| w * psi[n.index()])
                        .sum();
                    let total: f64 = weights.iter().sum();
                    if total > 0.0 {
                        sum / total
                    } else {
                        psi[i]
                    }
                }
            };
        });
        std::mem::swap(psi, &mut next);

        let residual = residual_inf(adjacency, fixed, psi, scale);
        if residual < options.tol {
            return Ok((iter, residual));
        }
    }
    Err(SolverError::NotConverged {
        iters: options.max_iters,
        residual: residual_inf(adjacency, fixed, psi, scale),
    })
    // SOLUTION-END
}

/// Fonction de courant résolue aux sommets du maillage.
///
/// Ce type n'implémente **pas** [`crate::velocity::VelocityField`] : il ne connaît `ψ`
/// qu'aux sommets, et ne saurait pas répondre en un point quelconque. Il implémente le
/// contrat plus pauvre dont le solveur a réellement besoin, [`StreamSource`].
#[derive(Clone, Debug)]
pub struct ComputedStream {
    psi: Vec<f64>,
}

impl ComputedStream {
    /// Résout `∇²ψ = 0` sur le maillage. Renvoie aussi les itérations et le résidu.
    pub fn solve(
        mesh: &Mesh,
        options: &StreamOptions,
    ) -> Result<(ComputedStream, usize, f64), SolverError> {
        let adjacency = adjacency(mesh);
        let fixed = boundary_values(mesh, options);
        let scale = scale_of(mesh, options);

        // Partir de la valeur imposée là où il y en a une, de zéro ailleurs.
        let mut psi: Vec<f64> = fixed.iter().map(|v| v.unwrap_or(0.0)).collect();
        let (iters, residual) = jacobi(&adjacency, &fixed, &mut psi, scale, options)?;
        Ok((ComputedStream { psi }, iters, residual))
    }

    /// La fonction de courant, indexée comme les sommets du maillage.
    pub fn psi(&self) -> &[f64] {
        &self.psi
    }
}

impl StreamSource for ComputedStream {
    fn stream_at(&self, id: VertexId, _p: Point) -> f64 {
        self.psi[id.index()]
    }
}

/// Échelle de `ψ` : le débit total de la veine, ce qui rend le résidu sans dimension.
fn scale_of(mesh: &Mesh, options: &StreamOptions) -> f64 {
    let (_, ymin, _, ymax) = mesh.bounds();
    (options.speed * (ymax - ymin)).abs().max(f64::MIN_POSITIVE)
}

/// Résidu du système pour un champ `ψ` donné, sans rien résoudre.
///
/// Sert à vérifier qu'un champ *connu* satisfait bien le schéma : dans une veine vide,
/// `ψ₀` en est la solution exacte, et son résidu doit être nul à la précision machine.
pub fn residual_of(mesh: &Mesh, options: &StreamOptions, psi: &[f64]) -> f64 {
    let adjacency = adjacency(mesh);
    let fixed = boundary_values(mesh, options);
    residual_inf(&adjacency, &fixed, psi, scale_of(mesh, options))
}
