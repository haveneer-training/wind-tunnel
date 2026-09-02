//! Compte les allocations de chaque phase du calcul.
//!
//! `cargo run --release --example alloc_count`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

/// Un allocateur qui délègue tout à celui du système, en comptant au passage.
struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(l.size(), Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(n, Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn snapshot() -> (usize, usize) {
    (ALLOCS.load(Relaxed), BYTES.load(Relaxed))
}

fn report(label: &str, before: (usize, usize), after: (usize, usize)) {
    println!(
        "{label:<28} {:>9} allocations {:>8} Ko",
        after.0 - before.0,
        (after.1 - before.1) / 1024
    );
}

fn main() {
    use wind_tunnel::field::Field;
    use wind_tunnel::flux::Upwind;
    use wind_tunnel::geom::Point;
    use wind_tunnel::io::{png, vtk};
    use wind_tunnel::mask::Mask;
    use wind_tunnel::mesh::Mesh;
    use wind_tunnel::solver::{Config, Solver, TimeScheme};
    use wind_tunnel::velocity::PotentialCylinder;

    let mask = Mask::from_file("domains/tunnel.dom").unwrap().refine(4);
    let h = 0.25;

    let t = snapshot();
    let mesh = Mesh::from_mask(&mask, h).unwrap();
    let u = snapshot();
    println!("{} cellules, {} faces", mesh.n_cells(), mesh.n_faces());
    report("Mesh::from_mask", t, u);

    let flow = PotentialCylinder {
        center: Point::new(10.0, 5.0),
        radius: 1.5,
        speed: 1.0,
        circulation: 0.0,
    };
    let mut c = Field::filled(mesh.n_cells(), 1.0);
    let mut work = Field::zeros(mesh.n_cells());

    for scheme in [TimeScheme::Euler, TimeScheme::Rk2] {
        let config = Config {
            steps: 20,
            output_every: 0,
            check_every: 0,
            time_scheme: scheme,
            ..Config::default()
        };
        let solver = Solver::new(&mesh, &flow, Upwind, config).unwrap();
        solver.step(&mut c, &mut work); // chauffe
        let t = snapshot();
        for _ in 0..20 {
            solver.step(&mut c, &mut work);
        }
        let u = snapshot();
        report(&format!("20 pas {scheme:?}"), t, u);
    }

    let dir = std::env::temp_dir();
    let t = snapshot();
    vtk::write_vtk(dir.join("alloc.vtk"), &mesh, &[("c", &c)]).unwrap();
    let u = snapshot();
    report("1 fichier VTK", t, u);

    let t = snapshot();
    png::write_png(dir.join("alloc.png"), &mesh, &c, 900, (0.0, 1.0)).unwrap();
    let u = snapshot();
    report("1 image PNG", t, u);
}
