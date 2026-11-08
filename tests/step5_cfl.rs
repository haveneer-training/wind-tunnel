#![cfg(feature = "step5")]

//! Un pas de temps trop grand pour la condition CFL est refusé à la construction du
//! solveur, avant le moindre calcul : mieux vaut une erreur qu'un champ qui explose.

use wind_tunnel::error::SolverError;
use wind_tunnel::flux::Upwind;
use wind_tunnel::geom::Vec2;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::{Config, Solver};
use wind_tunnel::velocity::Uniform;

#[test]
fn unstable_time_step_is_rejected_before_computing() {
    let mask = Mask::parse("....\n....\n").unwrap();
    let mesh = Mesh::from_mask(&mask, 1.0).unwrap();
    let flow = Uniform {
        value: Vec2::new(1.0, 0.0),
    };
    let config = Config {
        dt: Some(42.0),
        ..Config::default()
    };
    match Solver::new(&mesh, &flow, Upwind, config) {
        Err(SolverError::Cfl { dt, dt_max }) => {
            assert_eq!(dt, 42.0);
            assert!(dt_max < dt, "dt_max = {dt_max}");
        }
        Err(other) => panic!("attendu Cfl, obtenu {other}"),
        Ok(_) => panic!("le solveur a accepté un pas de temps instable"),
    }
}
