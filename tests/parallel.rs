#![cfg(feature = "step9")]

//! La parallélisation de l'étape 9 ne change aucune réduction flottante : chaque cellule
//! écrit sa propre case, et rien n'est sommé dans un ordre qui dépendrait du nombre de
//! threads. Le résultat doit donc être identique au bit près, qu'on force un seul thread
//! ou plusieurs — sinon, c'est qu'une écriture partagée s'est glissée quelque part.

use wind_tunnel::field::Field;
use wind_tunnel::flux::Muscl;
use wind_tunnel::geom::Vec2;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::{Config, Solver, TimeScheme};
use wind_tunnel::velocity::Uniform;

fn run_on(threads: usize) -> Field {
    let mask = Mask::parse(
        "............\n\
         ............\n\
         ............\n\
         ............\n\
         ............\n\
         ............\n",
    )
    .unwrap()
    .refine(3);
    let mesh = Mesh::from_mask(&mask, 1.0).unwrap();
    let flow = Uniform {
        value: Vec2::new(1.0, 0.3),
    };
    let config = Config {
        steps: 60,
        output_every: 0,
        time_scheme: TimeScheme::Rk2,
        ..Config::default()
    };
    let solver = Solver::new(&mesh, &flow, Muscl, config).unwrap();
    let mut c = Field::from_fn(&mesh, |id| {
        if mesh.cell(id).centroid.x < 3.0 {
            1.0
        } else {
            0.0
        }
    });

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    pool.install(|| solver.run(&mut c, |_, _, _| Ok(())).unwrap());
    c
}

#[test]
fn parallel_execution_matches_sequential_bit_for_bit() {
    let sequential = run_on(1);
    let parallel = run_on(4);
    assert_eq!(
        sequential, parallel,
        "le calcul parallèle diverge du séquentiel : une écriture partagée s'est glissée \
         quelque part"
    );
}
