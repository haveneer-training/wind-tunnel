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
///
/// Public pour `solutions/`, qui résout le même système par d'autres méthodes.
pub struct Adjacency {
    offsets: Vec<u32>,
    neighbours: Vec<VertexId>,
    weights: Vec<f64>,
}

impl Adjacency {
    /// Voisins du sommet `id`, et le poids de chaque arête.
    pub fn of(&self, id: VertexId) -> (&[VertexId], &[f64]) {
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
pub fn adjacency(mesh: &Mesh) -> Adjacency {
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
}

/// Niveau constant de la fonction de courant imposé sur l'obstacle.
pub fn obstacle_level(mesh: &Mesh, options: &StreamOptions) -> Option<f64> {
    // TODO-STEP:12 Calculer la valeur constante de ψ sur l'obstacle à partir de
    // l'ordonnée moyenne de ses sommets. Renvoyer `None` s'il n'y a pas d'obstacle.
    // HINT :
    //     ψ_obstacle = speed · (ȳ − ymin)
    // ȳ : moyenne des `y` des deux sommets de chaque face du groupe `BoundaryKind::Obstacle`
    // (`mesh.groups().get(…)?` rend `None` pour vous) ; `ymin` : 2ᵉ valeur de `mesh.bounds()`.
    // SOLUTION-BEGIN
    let (_, ymin, _, _) = mesh.bounds();
    let groups = mesh.groups();
    let faces = groups.get(&BoundaryKind::Obstacle)?;
    let (sum, count) = faces.iter().fold((0.0, 0.0), |(sum, count), id| {
        let face = mesh.face(*id);
        let a = mesh.vertices()[face.a.index()].y;
        let b = mesh.vertices()[face.b.index()].y;
        (sum + a + b, count + 2.0)
    });
    Some(options.speed * (sum / count - ymin))
    // SOLUTION-END
}

/// Valeur imposée à chaque sommet, `None` si elle est libre.
///
/// L'entrée et les parois portent `ψ₀(y) = speed × (y − ymin)`, l'obstacle une constante
/// donnée par [`obstacle_level`], et la sortie reste libre (condition de Neumann).
pub fn boundary_values(mesh: &Mesh, options: &StreamOptions) -> Vec<Option<f64>> {
    // TODO-STEP:12 Initialiser tous les sommets libres, fixer l'obstacle à sa valeur
    // constante, puis l'entrée et les parois avec ψ₀.
    // HINT : ψ₀(p) = speed · (p.y − ymin) s'écrit en fermeture `|p: Point| …` et se passe
    // telle quelle à `fix_group` ; départ : `vec![None; mesh.n_vertices()]`.
    // SOLUTION-BEGIN
    let (_, ymin, _, _) = mesh.bounds();
    let psi0 = |p: Point| options.speed * (p.y - ymin);
    let mut fixed = vec![None; mesh.n_vertices()];
    if let Some(value) = obstacle_level(mesh, options) {
        fix_group(mesh, &mut fixed, BoundaryKind::Obstacle, |_| value);
    }
    fix_group(mesh, &mut fixed, BoundaryKind::Inlet, psi0);
    fix_group(mesh, &mut fixed, BoundaryKind::Wall, psi0);
    fixed
    // SOLUTION-END
}

/// Applique une fonction de valeur imposée à tous les sommets d'un groupe de bord.
pub fn fix_group(
    mesh: &Mesh,
    fixed: &mut [Option<f64>],
    kind: BoundaryKind,
    value: impl Fn(Point) -> f64,
) {
    // TODO-STEP:12 Parcourir les faces du groupe et imposer la valeur calculée
    // aux deux sommets de chaque face.
    // HINT : `mesh.groups().get(&kind)` est une `Option<&Vec<FaceId>>` ;
    // `.into_iter().flatten()` parcourt ses faces, ou rien si le groupe n'existe pas.
    // SOLUTION-BEGIN
    let groups = mesh.groups();
    for id in groups.get(&kind).into_iter().flatten() {
        let face = mesh.face(*id);
        for vertex in [face.a, face.b] {
            fixed[vertex.index()] = Some(value(mesh.vertices()[vertex.index()]));
        }
    }
    // SOLUTION-END
}

/// Résidu `max |Σ w (ψ_voisin − ψ_v)|` sur les sommets libres, rapporté à l'échelle de `ψ`.
pub fn residual_inf(adjacency: &Adjacency, fixed: &[Option<f64>], psi: &[f64], scale: f64) -> f64 {
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

/// Effectue un seul balayage de Jacobi dans le tampon `next`.
fn sweep(adjacency: &Adjacency, fixed: &[Option<f64>], psi: &[f64], next: &mut [f64]) {
    // TODO-STEP:12 En parallèle, recopier les valeurs imposées et déléguer le calcul
    // des valeurs libres à `weighted_mean`.
    // HINT : `next.par_iter_mut().enumerate()` ; `fixed[i].unwrap_or_else(|| …)`.
    // SOLUTION-BEGIN
    next.par_iter_mut().enumerate().for_each(|(i, slot)| {
        *slot = fixed[i].unwrap_or_else(|| weighted_mean(adjacency, psi, i));
    });
    // SOLUTION-END
}

/// Moyenne des valeurs voisines pondérée par les poids du graphe.
fn weighted_mean(adjacency: &Adjacency, psi: &[f64], i: usize) -> f64 {
    // TODO-STEP:12 Calculer la somme pondérée et son poids total ; conserver `psi[i]`
    // si le sommet n'a aucun poids incident.
    // HINT : `adjacency.of(VertexId(i as u32))` rend `(voisins, poids)` ;
    //     ψ_i = Σ wₖ·ψ[voisinₖ] / Σ wₖ        (Σ wₖ = 0 → garder psi[i])
    // SOLUTION-BEGIN
    let (neighbours, weights) = adjacency.of(VertexId(i as u32));
    let total: f64 = weights.iter().sum();
    if total == 0.0 {
        psi[i]
    } else {
        neighbours
            .iter()
            .zip(weights)
            .map(|(n, w)| w * psi[n.index()])
            .sum::<f64>()
            / total
    }
    // SOLUTION-END
}

/// Balayages de Jacobi jusqu'au résidu visé. Renvoie le nombre d'itérations et le résidu.
///
/// Un balayage remplace la valeur de chaque sommet libre par la moyenne pondérée de ses
/// voisins, `ψ_v ← (Σ w ψ_voisin) / (Σ w)` ; les sommets imposés ne bougent pas. On lit
/// `psi`, on écrit `next`, puis on échange les deux — jamais sur place, sinon le résultat
/// dépend de l'ordre de parcours et la version parallèle ne serait pas reproductible.
fn jacobi(
    adjacency: &Adjacency,
    fixed: &[Option<f64>],
    psi: &mut Vec<f64>,
    scale: f64,
    options: &StreamOptions,
) -> Result<(usize, f64), SolverError> {
    // TODO-STEP:12 Cloner un tampon de travail, répéter le balayage et l'échange des
    // tampons, tester le résidu après chaque itération, puis signaler la non-convergence.
    // HINT : `std::mem::swap(psi, &mut next)` échange les deux tampons sans rien copier.
    // SOLUTION-BEGIN
    let mut next = psi.clone();
    for iter in 1..=options.max_iters {
        sweep(adjacency, fixed, psi, &mut next);
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
pub fn scale_of(mesh: &Mesh, options: &StreamOptions) -> f64 {
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

#[cfg(all(test, feature = "step12"))]
mod tests {
    use super::*;
    use crate::mask::Mask;

    fn mesh_of(text: &str) -> Mesh {
        Mesh::from_mask(&Mask::parse(text).unwrap(), 1.0).unwrap()
    }

    #[test]
    fn obstacle_level_is_constant_and_missing_without_an_obstacle() {
        let options = StreamOptions {
            speed: 2.0,
            ..StreamOptions::default()
        };
        let mesh = mesh_of("....\n..#.\n....\n");
        assert_eq!(obstacle_level(&mesh, &options), Some(3.0));
        assert_eq!(obstacle_level(&mesh_of("....\n....\n"), &options), None);
    }

    #[test]
    fn boundary_values_fix_solid_walls_and_leave_the_outlet_free() {
        let mesh = mesh_of("....\n..#.\n....\n");
        let fixed = boundary_values(&mesh, &StreamOptions::default());
        let groups = mesh.groups();
        let obstacle_values: Vec<f64> = groups[&BoundaryKind::Obstacle]
            .iter()
            .flat_map(|id| {
                let face = mesh.face(*id);
                [
                    fixed[face.a.index()].unwrap(),
                    fixed[face.b.index()].unwrap(),
                ]
            })
            .collect();
        assert!(
            obstacle_values
                .iter()
                .all(|value| *value == obstacle_values[0])
        );
        let outlet_middle = mesh.face(groups[&BoundaryKind::Outlet][1]);
        assert_eq!(fixed[outlet_middle.a.index()], None);
        assert_eq!(fixed[outlet_middle.b.index()], None);
    }

    #[test]
    fn sweep_preserves_fixed_values_and_updates_free_vertices() {
        let mesh = mesh_of("..\n..\n");
        let adjacency = adjacency(&mesh);
        let mut fixed = vec![None; mesh.n_vertices()];
        fixed[0] = Some(17.0);
        let psi: Vec<f64> = (0..mesh.n_vertices()).map(|i| i as f64).collect();
        let mut next = vec![0.0; psi.len()];
        sweep(&adjacency, &fixed, &psi, &mut next);
        assert_eq!(next[0], 17.0);
        for i in 1..psi.len() {
            let (neighbours, weights) = adjacency.of(VertexId(i as u32));
            let sum: f64 = neighbours
                .iter()
                .zip(weights)
                .map(|(id, weight)| weight * psi[id.index()])
                .sum();
            let total: f64 = weights.iter().sum();
            let expected = if total == 0.0 { psi[i] } else { sum / total };
            assert!((next[i] - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn fix_group_applies_a_value_to_each_endpoint() {
        let mesh = mesh_of("...\n...\n");
        let mut fixed = vec![None; mesh.n_vertices()];
        fix_group(&mesh, &mut fixed, BoundaryKind::Wall, |point| point.y);
        let groups = mesh.groups();
        for id in &groups[&BoundaryKind::Wall] {
            let face = mesh.face(*id);
            for vertex in [face.a, face.b] {
                assert_eq!(
                    fixed[vertex.index()],
                    Some(mesh.vertices()[vertex.index()].y)
                );
            }
        }
    }

    #[test]
    fn weighted_mean_handles_weighted_neighbours_and_isolated_vertices() {
        let adjacency = Adjacency {
            offsets: vec![0, 2, 2, 2],
            neighbours: vec![VertexId(1), VertexId(2)],
            weights: vec![1.0, 3.0],
        };
        assert_eq!(weighted_mean(&adjacency, &[0.0, 4.0, 8.0], 0), 7.0);

        let isolated = Adjacency {
            offsets: vec![0, 0],
            neighbours: Vec::new(),
            weights: Vec::new(),
        };
        assert_eq!(weighted_mean(&isolated, &[9.0], 0), 9.0);
    }
}
