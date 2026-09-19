#![cfg(feature = "step12")]

//! Propriétés de l'écoulement calculé : étanchéité des parois, conservation, convergence.

use wind_tunnel::error::SolverError;
use wind_tunnel::flux::Upwind;
use wind_tunnel::geom::Point;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::{BoundaryKind, Mesh};
use wind_tunnel::solver::{Bc, Config, Solver};
use wind_tunnel::stream::{residual_of, ComputedStream, StreamOptions};
use wind_tunnel::velocity::PotentialCylinder;

const H: f64 = 2.0;

fn mesh_of(path: &str) -> Mesh {
    let mask = Mask::from_file(path).unwrap_or_else(|e| panic!("{path} : {e}"));
    Mesh::from_mask(&mask, H).expect("maillage impossible")
}

/// Réglages volontairement lâches : les propriétés d'étanchéité et de conservation ne
/// dépendent **pas** de la convergence — elles tiennent à la forme du schéma, pas à la
/// qualité de la solution. Un résidu grossier suffit donc, et les tests restent rapides.
fn options() -> StreamOptions {
    StreamOptions {
        speed: 1.0,
        max_iters: 100_000,
        tol: 1e-3,
    }
}

/// Une veine vide de 20 × 8 cases, assez petite pour qu'un Jacobi y converge vraiment.
fn small_channel() -> Mesh {
    let rows = ["........................"; 8].join("\n");
    let mask = Mask::parse(&rows).expect("masque");
    Mesh::from_mask(&mask, H).expect("maillage impossible")
}

/// Configuration avec des parois réellement imperméables : c'est ce que l'écoulement
/// calculé autorise, et que l'écoulement analytique interdit.
fn tight_config() -> Config {
    let mut config = Config::default();
    config.bc.insert(BoundaryKind::Wall, Bc::NoFlux);
    config.bc.insert(BoundaryKind::Obstacle, Bc::NoFlux);
    config
}

/// Débit maximal traversant un groupe de bord, pour un écoulement donné.
fn worst_boundary_flux<S: wind_tunnel::velocity::StreamSource + ?Sized>(
    mesh: &Mesh,
    flow: &S,
    kinds: &[BoundaryKind],
) -> f64 {
    let solver = Solver::new(mesh, flow, Upwind, tight_config()).expect("solveur");
    let mut worst = 0.0f64;
    for kind in kinds {
        for id in mesh.groups().get(kind).into_iter().flatten() {
            worst = worst.max(solver.face_flux(*id).abs());
        }
    }
    worst
}

#[test]
fn walls_are_exact_streamlines() {
    let mesh = mesh_of("domains/square.dom");
    let (stream, _, _) = ComputedStream::solve(&mesh, &options()).expect("résolution");
    let worst = worst_boundary_flux(
        &mesh,
        &stream,
        &[BoundaryKind::Wall, BoundaryKind::Obstacle],
    );
    // Pas « petit » : nul. Les deux extrémités d'une face de paroi portent le *même*
    // `f64`, donc leur différence est exactement `0.0`.
    assert_eq!(worst, 0.0, "une paroi laisse passer un débit de {worst:e}");
}

#[test]
fn the_analytic_flow_leaks_through_a_square_obstacle() {
    let mesh = mesh_of("domains/square.dom");
    let (_, ymin, _, ymax) = mesh.bounds();
    // Le disque de même aire que le carré, celui que l'écoulement analytique contourne.
    let side = 14.0 * H;
    let flow = PotentialCylinder {
        center: Point::new(34.0 * H, 0.5 * (ymin + ymax)),
        radius: (side * side / std::f64::consts::PI).sqrt(),
        speed: 1.0,
        circulation: 0.0,
    };
    let worst = worst_boundary_flux(&mesh, &flow, &[BoundaryKind::Obstacle]);
    // C'est toute la raison d'être de l'étape : la formule ne connaît pas la forme.
    assert!(
        worst > 1e-3 * H,
        "l'écoulement analytique devrait fuir à travers un obstacle carré, débit {worst:e}"
    );
}

#[test]
fn a_square_obstacle_keeps_the_divergence_at_zero() {
    let mesh = mesh_of("domains/square.dom");
    let (stream, _, _) = ComputedStream::solve(&mesh, &options()).expect("résolution");
    let solver = Solver::new(&mesh, &stream, Upwind, tight_config()).expect("solveur");
    let worst = (0..mesh.n_cells())
        .map(|i| solver.divergence(wind_tunnel::mesh::CellId(i as u32)).abs())
        .fold(0.0f64, f64::max);
    assert!(worst < 1e-12, "divergence {worst:e}");
}

#[test]
fn the_dual_laplacian_annihilates_a_uniform_flow() {
    // Veine vide : l'intérieur n'est que des quadrangles réguliers, sans chanfrein, et
    // `ψ₀(y) = speed × (y − ymin)` est alors la solution exacte du système discret.
    let mesh = mesh_of("domains/tunnel-empty.dom");
    let (_, ymin, _, _) = mesh.bounds();
    let options = options();
    let psi: Vec<f64> = mesh
        .vertices()
        .iter()
        .map(|p| options.speed * (p.y - ymin))
        .collect();
    let residual = residual_of(&mesh, &options, &psi);
    assert!(residual < 1e-12, "résidu de ψ₀ : {residual:e}");
}

#[test]
fn jacobi_converges_to_the_uniform_flow() {
    let mesh = small_channel();
    let (_, ymin, _, _) = mesh.bounds();
    let options = StreamOptions {
        tol: 1e-10,
        ..options()
    };
    let (stream, iters, residual) = ComputedStream::solve(&mesh, &options).expect("résolution");
    assert!(iters >= 1 && residual < options.tol);

    let worst = mesh
        .vertices()
        .iter()
        .zip(stream.psi())
        .map(|(p, psi)| (psi - options.speed * (p.y - ymin)).abs())
        .fold(0.0f64, f64::max);
    // Le résidu n'est pas l'erreur : pour Jacobi, l'un vaut environ `(1 − ρ)` fois
    // l'autre, et `1 − ρ` se dégrade comme le carré de la taille du maillage. C'est
    // pour ça qu'un résidu à 1e-10 ne garantit ici qu'une erreur à 1e-7 — et c'est ce
    // que le gradient conjugué de l'extension corrige.
    assert!(worst < 1e-6, "écart à ψ₀ : {worst:e}");
}

#[test]
fn a_tiny_iteration_budget_reports_no_convergence() {
    let mesh = mesh_of("domains/square.dom");
    let options = StreamOptions {
        max_iters: 5,
        ..options()
    };
    match ComputedStream::solve(&mesh, &options) {
        Err(SolverError::NotConverged { iters, residual }) => {
            assert_eq!(iters, 5);
            assert!(residual > options.tol);
        }
        other => panic!("attendu NotConverged, obtenu {other:?}"),
    }
}
