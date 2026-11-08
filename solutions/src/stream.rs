//! Étape 12, « pour aller plus loin » : résoudre `∇²ψ = 0` autrement que par Jacobi.
//!
//! - [`conjugate_gradient`] — le gradient conjugué *matrix-free* : quelques centaines
//!   d'itérations là où Jacobi en demande des dizaines de milliers ;
//! - [`sor`] — la surrelaxation successive : bien plus rapide que Jacobi, mais
//!   séquentielle par nature, le résultat dépendant de l'ordre de parcours ;
//! - [`zero_circulation_level`] — la constante de l'obstacle qui annule la circulation
//!   autour du corps, par superposition de deux résolutions.
//!
//! Le système est celui du corrigé, repris tel quel : voisinage (`adjacency`), valeurs
//! imposées (`boundary_values`, `fix_group`), constante de l'obstacle (`obstacle_level`)
//! et résidu (`residual_inf`) viennent de `wind_tunnel::stream`. Seules les méthodes de
//! résolution sont écrites ici.
//!
//! `cargo run --release --manifest-path solutions/Cargo.toml --example stream_solvers`
//! compare les itérations des trois méthodes à plusieurs raffinements.

use wind_tunnel::error::SolverError;
use wind_tunnel::geom::Point;
use wind_tunnel::mesh::{BoundaryKind, Mesh, VertexId};
use wind_tunnel::stream::{
    Adjacency, StreamOptions, adjacency, boundary_values, fix_group, residual_inf, scale_of,
};

pub use wind_tunnel::stream::obstacle_level;

/// Le système `Σ w (ψ_voisin − ψ_v) = 0` aux sommets libres, et les valeurs imposées.
pub struct System {
    adjacency: Adjacency,
    /// Valeur imposée de chaque sommet, `None` s'il est libre.
    fixed: Vec<Option<f64>>,
    /// Débit total de la veine : rend le résidu sans dimension.
    scale: f64,
}

impl System {
    /// Le système de l'étape 12, valeurs imposées comprises.
    pub fn new(mesh: &Mesh, options: &StreamOptions) -> System {
        System {
            adjacency: adjacency(mesh),
            fixed: boundary_values(mesh, options),
            scale: scale_of(mesh, options),
        }
    }

    /// Le même système, avec `wall(p)` sur l'entrée et les parois, et `obstacle` sur le
    /// corps. Même ordre que `boundary_values` : l'obstacle d'abord, que l'entrée et les
    /// parois remplacent sur un sommet partagé.
    pub fn with_values(
        mesh: &Mesh,
        options: &StreamOptions,
        wall: impl Fn(Point) -> f64,
        obstacle: f64,
    ) -> System {
        let mut fixed = vec![None; mesh.n_vertices()];
        fix_group(mesh, &mut fixed, BoundaryKind::Obstacle, |_| obstacle);
        fix_group(mesh, &mut fixed, BoundaryKind::Inlet, &wall);
        fix_group(mesh, &mut fixed, BoundaryKind::Wall, &wall);
        System {
            adjacency: adjacency(mesh),
            fixed,
            scale: scale_of(mesh, options),
        }
    }

    /// Le point de départ du corrigé : la valeur imposée là où il y en a une, zéro ailleurs.
    pub fn start(&self) -> Vec<f64> {
        self.fixed.iter().map(|v| v.unwrap_or(0.0)).collect()
    }

    /// Les arêtes du sommet `i` : `(voisin, poids)`.
    fn edges(&self, i: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let (neighbours, weights) = self.adjacency.of(VertexId(i as u32));
        neighbours
            .iter()
            .map(|n| n.index())
            .zip(weights.iter().copied())
    }

    /// Résidu de chaque sommet, `Σ w (ψ_voisin − ψ_v)`, nul sur les sommets imposés.
    fn residual(&self, psi: &[f64], out: &mut [f64]) {
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = 0.0;
            if self.fixed[i].is_none() {
                for (j, w) in self.edges(i) {
                    *slot += w * (psi[j] - psi[i]);
                }
            }
        }
    }

    /// Le résidu relatif, en norme infinie — celui du corrigé.
    pub fn residual_inf(&self, psi: &[f64]) -> f64 {
        residual_inf(&self.adjacency, &self.fixed, psi, self.scale)
    }

    /// Le produit `A p` restreint aux sommets libres : une direction de descente ne
    /// touche jamais une valeur imposée, qui compte donc pour zéro.
    fn apply(&self, p: &[f64], out: &mut [f64]) {
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = 0.0;
            if self.fixed[i].is_none() {
                for (j, w) in self.edges(i) {
                    let pj = if self.fixed[j].is_none() { p[j] } else { 0.0 };
                    *slot += w * (p[i] - pj);
                }
            }
        }
    }

    /// Le défaut de circulation autour de l'obstacle : `Σ w (ψ_voisin − ψ_v)` sur ses
    /// sommets. Nul quand le débit qui passe au-dessus du corps et celui qui passe
    /// au-dessous se répartissent sans tourbillon autour de lui.
    pub fn circulation(&self, mesh: &Mesh, psi: &[f64]) -> f64 {
        // Les sommets de l'obstacle, chacun une fois : marqués avec `fix_group`.
        let mut on_body = vec![None; mesh.n_vertices()];
        fix_group(mesh, &mut on_body, BoundaryKind::Obstacle, |_| 1.0);
        let mut total = 0.0;
        for v in (0..on_body.len()).filter(|&v| on_body[v].is_some()) {
            for (j, w) in self.edges(v) {
                total += w * (psi[j] - psi[v]);
            }
        }
        total
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut sum = 0.0;
    for i in 0..a.len() {
        sum += a[i] * b[i];
    }
    sum
}

fn norm_inf(a: &[f64]) -> f64 {
    let mut worst = 0.0_f64;
    for &x in a {
        worst = worst.max(x.abs());
    }
    worst
}

/// Gradient conjugué *matrix-free*.
///
/// La matrice n'est jamais écrite : `apply` est le corps du balayage de Jacobi, lu comme
/// un produit matrice-vecteur. Le système est symétrique (une arête porte le même poids
/// dans les deux sens) et défini positif (chaque composante libre touche une valeur
/// imposée) : c'est exactement le cadre du gradient conjugué. Renvoie `ψ`, le nombre
/// d'itérations et le résidu atteint.
pub fn conjugate_gradient(
    system: &System,
    options: &StreamOptions,
) -> Result<(Vec<f64>, usize, f64), SolverError> {
    let mut psi = system.start();
    let n = psi.len();
    // r = b − A ψ, c'est-à-dire le résidu du schéma ; nul sur les sommets imposés.
    let mut r = vec![0.0; n];
    system.residual(&psi, &mut r);
    let mut p = r.clone();
    let mut q = vec![0.0; n];
    let mut rr = dot(&r, &r);

    for iter in 1..=options.max_iters {
        if norm_inf(&r) / system.scale < options.tol {
            let residual = system.residual_inf(&psi);
            return Ok((psi, iter - 1, residual));
        }
        system.apply(&p, &mut q);
        let alpha = rr / dot(&p, &q);
        for i in 0..n {
            psi[i] += alpha * p[i];
            r[i] -= alpha * q[i];
        }
        let rr_next = dot(&r, &r);
        let beta = rr_next / rr;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rr = rr_next;
    }
    Err(SolverError::NotConverged {
        iters: options.max_iters,
        residual: system.residual_inf(&psi),
    })
}

/// Surrelaxation successive : `ψ_v ← ψ_v + ω (moyenne − ψ_v)`, **sur place**.
///
/// Chaque sommet lit les valeurs que le même balayage vient d'écrire chez ses voisins.
/// C'est ce qui accélère la convergence, et ce qui interdit de paralléliser le balayage
/// tel quel : l'opposé exact de la forme *gather* à deux tampons du reste du projet.
pub fn sor(
    system: &System,
    options: &StreamOptions,
    omega: f64,
) -> Result<(Vec<f64>, usize, f64), SolverError> {
    let mut psi = system.start();
    for iter in 1..=options.max_iters {
        for i in 0..psi.len() {
            if system.fixed[i].is_some() {
                continue;
            }
            let mut sum = 0.0;
            let mut total = 0.0;
            for (j, w) in system.edges(i) {
                sum += w * psi[j];
                total += w;
            }
            if total > 0.0 {
                psi[i] += omega * (sum / total - psi[i]);
            }
        }
        let residual = system.residual_inf(&psi);
        if residual < options.tol {
            return Ok((psi, iter, residual));
        }
    }
    Err(SolverError::NotConverged {
        iters: options.max_iters,
        residual: system.residual_inf(&psi),
    })
}

/// La constante de l'obstacle qui annule la circulation autour du corps.
///
/// Le problème est linéaire en ses valeurs imposées. Avec `ψ_a` (parois à `ψ₀`, obstacle
/// à 0) et `ψ_b` (parois à 0, obstacle à 1), la solution pour une constante `c` vaut
/// `ψ_a + c ψ_b`, et sa circulation `Γ_a + c Γ_b`. Elle s'annule pour `c = −Γ_a / Γ_b` :
/// deux résolutions, une division. `None` s'il n'y a pas d'obstacle.
pub fn zero_circulation_level(
    mesh: &Mesh,
    options: &StreamOptions,
) -> Result<Option<f64>, SolverError> {
    if mesh.groups().get(&BoundaryKind::Obstacle).is_none() {
        return Ok(None);
    }
    let (_, ymin, _, _) = mesh.bounds();
    let a = System::with_values(mesh, options, |p| options.speed * (p.y - ymin), 0.0);
    let b = System::with_values(mesh, options, |_| 0.0, 1.0);
    // Les deux problèmes partiels ont pour échelle le débit de la veine, pas le leur :
    // une tolérance plus serrée garde le quotient précis.
    let tight = StreamOptions {
        tol: options.tol * 1e-3,
        ..*options
    };
    let (psi_a, _, _) = conjugate_gradient(&a, &tight)?;
    let (psi_b, _, _) = conjugate_gradient(&b, &tight)?;
    Ok(Some(
        -a.circulation(mesh, &psi_a) / b.circulation(mesh, &psi_b),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wind_tunnel::mask::Mask;
    use wind_tunnel::stream::{ComputedStream, residual_of};

    fn mesh_of(text: &str) -> Mesh {
        Mesh::from_mask(&Mask::parse(text).unwrap(), 1.0).unwrap()
    }

    fn tunnel() -> Mesh {
        let mask = Mask::from_file(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../domains/tunnel.dom"
        ))
        .expect("domains/tunnel.dom illisible");
        Mesh::from_mask(&mask, 1.0).unwrap()
    }

    #[test]
    fn cg_and_sor_converge_far_faster_than_jacobi() {
        let mesh = tunnel();
        let options = StreamOptions::default();
        let (_, jacobi, _) = ComputedStream::solve(&mesh, &options).unwrap();
        let system = System::new(&mesh, &options);

        let (psi, cg, _) = conjugate_gradient(&system, &options).unwrap();
        assert!(residual_of(&mesh, &options, &psi) < options.tol);
        assert!(cg * 10 < jacobi, "CG {cg} contre Jacobi {jacobi}");

        let (psi, sor_iters, _) = sor(&system, &options, 1.9).unwrap();
        assert!(residual_of(&mesh, &options, &psi) < options.tol);
        assert!(
            sor_iters * 3 < jacobi,
            "SOR {sor_iters} contre Jacobi {jacobi}"
        );
    }

    #[test]
    fn a_symmetric_body_needs_no_correction() {
        // Corps centré dans une veine symétrique : la constante de l'étape 12 est déjà la
        // bonne, la superposition doit la retrouver.
        let mesh =
            mesh_of("..........\n..........\n....##....\n....##....\n..........\n..........\n");
        let options = StreamOptions::default();
        let level = obstacle_level(&mesh, &options).unwrap();
        let c = zero_circulation_level(&mesh, &options).unwrap().unwrap();
        assert!((c - level).abs() < 1e-6, "{c} ≠ {level}");
    }

    #[test]
    fn an_off_axis_body_gets_its_own_constant() {
        let mesh =
            mesh_of("..........\n....##....\n....##....\n..........\n..........\n..........\n");
        let options = StreamOptions::default();
        let level = obstacle_level(&mesh, &options).unwrap();
        let c = zero_circulation_level(&mesh, &options).unwrap().unwrap();
        assert!((c - level).abs() > 1e-2, "{c} ≈ {level}");

        let (_, ymin, _, _) = mesh.bounds();
        let system = System::with_values(&mesh, &options, |p| options.speed * (p.y - ymin), c);
        let tight = StreamOptions {
            tol: 1e-10,
            ..options
        };
        let (psi, _, _) = conjugate_gradient(&system, &tight).unwrap();
        assert!(system.circulation(&mesh, &psi).abs() < 1e-6);
    }
}
