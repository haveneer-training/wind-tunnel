//! Rendu à verrou contre rendu sans verrou, à plusieurs tailles de maillage.
//!
//! `cargo run --release --manifest-path solutions/Cargo.toml --example render_timing`

use std::sync::Arc;
use std::time::Instant;

use wind_tunnel::app::initial_field;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel_solutions::render::{
    render_lock_free, render_rayon, render_sequential, render_spawn_arc,
};

/// Meilleur temps sur quelques essais, en millisecondes.
fn best_of(mut run: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..5 {
        let start = Instant::now();
        run();
        best = best.min(start.elapsed().as_secs_f64() * 1e3);
    }
    best
}

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../domains/tunnel.dom");
    let mask = Mask::from_file(path).expect("domains/tunnel.dom illisible");
    println!(
        "raffinement  cellules  largeur   séquentiel  spawn+Arc  sans verrou  rayon+Mutex  (ms)"
    );
    for (refine, width) in [(1, 600), (2, 1200), (4, 2400)] {
        let mesh = Mesh::from_mask(&mask.refine(refine), 1.0).expect("maillage impossible");
        let field = initial_field(&mesh, 9);
        let range = (0.0, 1.0);
        let seq = best_of(|| drop(render_sequential(&mesh, &field, width, range)));
        let (shared_mesh, shared_field) = (Arc::new(mesh.clone()), Arc::new(field.clone()));
        let spawn = best_of(|| {
            drop(render_spawn_arc(
                Arc::clone(&shared_mesh),
                Arc::clone(&shared_field),
                width,
                range,
            ))
        });
        let free = best_of(|| drop(render_lock_free(&mesh, &field, width, range)));
        let rayon = best_of(|| drop(render_rayon(&mesh, &field, width, range)));
        println!(
            "{refine:>11}  {:>8}  {width:>7}   {seq:>10.1}  {spawn:>9.1}  {free:>11.1}  {rayon:>11.1}",
            mesh.n_cells()
        );
    }
}
