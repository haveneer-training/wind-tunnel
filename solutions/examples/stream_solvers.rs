//! Jacobi, gradient conjugué et SOR sur le même système, à plusieurs raffinements.
//!
//! `cargo run --release --manifest-path solutions/Cargo.toml --example stream_solvers`

use std::time::Instant;

use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::stream::{ComputedStream, StreamOptions};
use wind_tunnel_solutions::stream::{
    System, conjugate_gradient, obstacle_level, sor, zero_circulation_level,
};

/// Un corps carré au quart supérieur d'une veine de 12 cases de haut.
const OFF_AXIS: &str = "\
........................
........................
.......####.............
.......####.............
.......####.............
........................
........................
........................
........................
........................
........................
........................
";

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../domains/tunnel.dom");
    let mask = Mask::from_file(path).expect("domains/tunnel.dom illisible");
    let options = StreamOptions::default();
    println!("domains/tunnel.dom, tolérance {:.0e}\n", options.tol);
    println!("raffinement  sommets   Jacobi (s)        CG (s)          SOR ω=1,9 (s)");
    for refine in [1, 2, 4] {
        let mesh = Mesh::from_mask(&mask.refine(refine), 1.0).expect("maillage impossible");
        let system = System::new(&mesh, &options);

        let start = Instant::now();
        let (_, jacobi, _) = ComputedStream::solve(&mesh, &options).expect("Jacobi");
        let t_jacobi = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let (_, cg, _) = conjugate_gradient(&system, &options).expect("CG");
        let t_cg = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let (_, sor_iters, _) = sor(&system, &options, 1.9).expect("SOR");
        let t_sor = start.elapsed().as_secs_f64();

        println!(
            "{refine:>11}  {:>7}  {jacobi:>7} ({t_jacobi:>5.2})  {cg:>6} ({t_cg:>5.2})  {sor_iters:>7} ({t_sor:>5.2})",
            mesh.n_vertices()
        );
    }

    for (name, text) in [
        (
            "tunnel.dom",
            std::fs::read_to_string(path).expect("domains/tunnel.dom illisible"),
        ),
        ("corps hors de l'axe", OFF_AXIS.to_string()),
    ] {
        let mesh = Mesh::from_mask(&Mask::parse(&text).expect("masque"), 1.0).expect("maillage");
        let level = obstacle_level(&mesh, &options).expect("pas d'obstacle");
        let c = zero_circulation_level(&mesh, &options)
            .expect("superposition")
            .expect("pas d'obstacle");
        println!(
            "\n{name} : constante de l'obstacle {level:.4} (ordonnée moyenne), \
             {c:.4} (circulation nulle)"
        );
    }
}
