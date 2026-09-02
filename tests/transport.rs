//! Propriétés du transport, de bout en bout sur le domaine livré.

use wind_tunnel::field::Field;
use wind_tunnel::flux::Upwind;
use wind_tunnel::geom::Point;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::{Config, Solver};
use wind_tunnel::velocity::PotentialCylinder;

const H: f64 = 0.5;

fn cas() -> (Mesh, PotentialCylinder) {
    let mask = Mask::from_file("domains/veine.dom").expect("domains/veine.dom illisible");
    let mesh = Mesh::from_mask(&mask, H).expect("maillage impossible");
    let (_, ymin, _, ymax) = mesh.bounds();
    let flow = PotentialCylinder {
        center: Point::new(34.5 * H, 0.5 * (ymin + ymax)),
        radius: 6.5 * H,
        speed: 1.0,
        circulation: 4.0,
    };
    (mesh, flow)
}

#[test]
fn les_debits_sont_a_divergence_nulle_partout() {
    let (mesh, flow) = cas();
    let solver = Solver::new(&mesh, &flow, Upwind, Config::default()).unwrap();
    let pire = (0..mesh.n_cells())
        .map(|i| solver.divergence(wind_tunnel::mesh::CellId(i as u32)).abs())
        .fold(0.0_f64, f64::max);
    assert!(pire < 1e-12, "divergence maximale {pire:e}");
}

#[test]
fn le_traceur_reste_entre_ses_bornes() {
    // le décentrement amont vérifie un principe du maximum : partant de valeurs dans
    // [0, 1], aucune cellule ne doit sortir de cet intervalle, jamais. Un dépassement
    // signalerait que le champ de débits n'est plus à divergence nulle.
    let (mesh, flow) = cas();
    let config = Config {
        steps: 300,
        output_every: 0,
        check_every: 25,
        ..Config::default()
    };
    let solver = Solver::new(&mesh, &flow, Upwind, config).unwrap();

    let (_, ymin, _, ymax) = mesh.bounds();
    let bande = (ymax - ymin) / 7.0;
    let mut c = Field::from_fn(&mesh, |id| {
        let y = mesh.cell(id).centroid.y;
        if ((y - ymin) / bande).floor() as i64 % 2 == 0 {
            1.0
        } else {
            0.0
        }
    });

    solver
        .run(&mut c, |_, _, champ| {
            let (lo, hi) = champ.min_max();
            assert!(lo >= -1e-12 && hi <= 1.0 + 1e-12, "c ∈ [{lo}, {hi}]");
            Ok(())
        })
        .unwrap();

    let (lo, hi) = c.min_max();
    assert!(lo >= -1e-12 && hi <= 1.0 + 1e-12, "c ∈ [{lo}, {hi}]");
}

#[test]
fn la_fumee_finit_par_sortir() {
    // sans injection en amont, le domaine se vide : la masse décroît strictement
    let (mesh, flow) = cas();
    let config = Config {
        steps: 200,
        output_every: 0,
        ..Config::default()
    };
    let solver = Solver::new(&mesh, &flow, Upwind, config).unwrap();
    let mut c = Field::filled(mesh.n_cells(), 1.0);
    let avant = c.total_mass(&mesh);
    solver.run(&mut c, |_, _, _| Ok(())).unwrap();
    let apres = c.total_mass(&mesh);
    assert!(apres < avant, "masse {avant} → {apres}");
    assert!(apres > 0.0);
}
