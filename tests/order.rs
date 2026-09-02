#![cfg(feature = "step6")]

//! Vérifie l'ordre du schéma par solution manufacturée : un profil lisse, transporté à
//! vitesse uniforme, a une solution exacte connue — sa propre translation. L'écart entre
//! le champ numérique et cette translation doit décroître comme h¹ : c'est ce qu'on
//! attend d'un schéma décentré amont couplé à un Euler explicite, tous deux d'ordre 1.
//! (L'étape 7 relève l'ordre spatial à 2 ; l'ordre mesuré ici n'en bougera pourtant pas
//! avant l'étape 8 — l'erreur en temps domine dès que `dt ∝ h`.)

use wind_tunnel::field::Field;
use wind_tunnel::flux::{FluxScheme, Upwind};
use wind_tunnel::geom::{Point, Vec2};
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::{Config, Solver, TimeScheme};
use wind_tunnel::velocity::Uniform;

const SPEED: f64 = 1.0;
const SIGMA: f64 = 0.6;
const CENTER0: Point = Point::new(3.0, 3.0);
const T_FINAL: f64 = 4.0; // durée physique du transport, fixée quel que soit le raffinement

fn bump(p: Point) -> f64 {
    let dx = p.x - CENTER0.x;
    let dy = p.y - CENTER0.y;
    (-(dx * dx + dy * dy) / (2.0 * SIGMA * SIGMA)).exp()
}

/// Écart entre le champ transporté numériquement et sa translation exacte, à un facteur
/// de raffinement donné du même domaine physique (voir [`Mask::refine`]), pour un
/// schéma de flux et une intégration en temps donnés.
fn advection_error(factor: usize, scheme: impl FluxScheme + Copy, time_scheme: TimeScheme) -> f64 {
    let base = Mask::parse(
        "............\n\
         ............\n\
         ............\n\
         ............\n\
         ............\n\
         ............\n",
    )
    .unwrap();
    let mask = base.refine(factor);
    let h = 1.0 / factor as f64;
    let mesh = Mesh::from_mask(&mask, h).unwrap();
    let flow = Uniform {
        value: Vec2::new(SPEED, 0.0),
    };

    let c0 = Field::from_fn(&mesh, |id| bump(mesh.cell(id).centroid));

    // pas de temps déduit du CFL par défaut : dt ∝ h, donc le nombre de pas croît avec
    // le raffinement pour couvrir la même durée physique T_FINAL
    let base_config = Config {
        time_scheme,
        ..Config::default()
    };
    let probe = Solver::new(&mesh, &flow, scheme, base_config.clone()).unwrap();
    let steps = (T_FINAL / probe.dt()).round() as usize;
    let config = Config {
        steps,
        output_every: 0,
        ..base_config
    };
    let solver = Solver::new(&mesh, &flow, scheme, config).unwrap();

    let mut c = c0.clone();
    solver.run(&mut c, |_, _, _| Ok(())).unwrap();

    let travelled = steps as f64 * solver.dt() * SPEED;
    let exact = Field::from_fn(&mesh, |id| {
        let p = mesh.cell(id).centroid;
        bump(Point::new(p.x - travelled, p.y))
    });
    c.mean_abs_error(&exact, &mesh)
}

#[test]
fn order_of_convergence_is_about_one() {
    let e_coarse = advection_error(8, Upwind, TimeScheme::Euler);
    let e_fine = advection_error(16, Upwind, TimeScheme::Euler);
    let order = (e_coarse / e_fine).ln() / 2f64.ln();
    assert!(
        (order - 1.0).abs() < 0.3,
        "ordre observé {order:.2} (attendu ≈ 1 : décentrement amont + Euler explicite, \
         tous deux d'ordre 1)"
    );
}

#[cfg(feature = "step8")]
#[test]
fn order_of_convergence_reaches_two_with_muscl_and_rk2() {
    use wind_tunnel::flux::Muscl;

    let e_coarse = advection_error(8, Muscl, TimeScheme::Rk2);
    let e_fine = advection_error(16, Muscl, TimeScheme::Rk2);
    let order = (e_coarse / e_fine).ln() / 2f64.ln();
    assert!(
        (order - 2.0).abs() < 0.4,
        "ordre observé {order:.2} (attendu ≈ 2 : reconstruction MUSCL + RK2, tous deux \
         d'ordre 2 — c'est ce qui résout l'énigme des étapes 6-7)"
    );
}
