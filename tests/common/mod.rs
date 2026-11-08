//! Aides partagées par les tests de plusieurs étapes.

#![allow(dead_code)] // chaque binaire de test n'utilise qu'une partie de ces aides

use std::path::PathBuf;

use wind_tunnel::field::Field;
use wind_tunnel::flux::FluxScheme;
use wind_tunnel::geom::{Point, Vec2};
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::Mesh;
use wind_tunnel::solver::{Config, Solver, TimeScheme};
use wind_tunnel::velocity::Uniform;

/// Un fichier temporaire propre à ce processus : deux `cargo test` lancés en même temps
/// sur la même machine n'écrivent pas dans le même fichier.
pub fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("wind-tunnel-{}-{name}", std::process::id()))
}

/// Un petit maillage avec un obstacle : 12 cellules au plus, de quoi relire un fichier.
pub fn small_mesh() -> Mesh {
    let mask = Mask::parse("....\n.##.\n....\n").expect("masque invalide");
    Mesh::from_mask(&mask, 0.5).expect("maillage impossible")
}

/// Valeur qui suit un mot-clé dans l'en-tête VTK, par exemple `POINTS 42 double`.
pub fn header_value(text: &str, keyword: &str) -> usize {
    text.lines()
        .find(|l| l.starts_with(keyword))
        .unwrap_or_else(|| panic!("mot-clé {keyword} absent du fichier"))
        .split_whitespace()
        .nth(1)
        .expect("valeur absente")
        .parse()
        .expect("valeur non entière")
}

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
///
/// C'est la solution manufacturée des étapes 6 et 8 : un profil lisse, transporté à
/// vitesse uniforme, a une solution exacte connue — sa propre translation.
pub fn advection_error(
    factor: usize,
    scheme: impl FluxScheme + Copy,
    time_scheme: TimeScheme,
) -> f64 {
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
        max_steps: steps,
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

/// L'ordre observé entre deux raffinements d'un facteur 2.
pub fn observed_order(scheme: impl FluxScheme + Copy, time_scheme: TimeScheme) -> f64 {
    let e_coarse = advection_error(8, scheme, time_scheme);
    let e_fine = advection_error(16, scheme, time_scheme);
    (e_coarse / e_fine).ln() / 2f64.ln()
}
