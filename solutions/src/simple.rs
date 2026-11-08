//! Variantes « simples » de trous du fil rouge.
//!
//! Le corrigé écrit volontiers des chaînes d'itérateurs (`map`, `fold`, `filter_map`)
//! et des `Option` dès les premières étapes, alors que le programme ne présente les
//! adaptateurs qu'au J3 et `Option` qu'au J2. Chaque fonction ci-dessous est la même
//! réponse écrite avec ce que le stagiaire connaît à ce moment-là : des boucles `for`,
//! des indices, des `if`. Les tests vérifient qu'elle rend **exactement** le même
//! résultat que le corrigé — au bit près quand l'ordre des opérations flottantes est le
//! même.
//!
//! Les trous du corrigé sont souvent privés ; leur variante est donc écrite ici en
//! fonction libre, sur l'API publique (`Mask::is_fluid`, `Solver::residual`, …).

use wind_tunnel::field::Field;
use wind_tunnel::geom::Point;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::{CellId, Mesh};

/// Étape 0, `polygon_area` : la formule du lacet, avec une boucle sur les indices.
///
/// L'arête `k` va du sommet `k` au sommet `k + 1`, et la dernière revient au premier :
/// c'est tout ce que fait `edges` dans le corrigé.
pub fn polygon_area(points: &[Point]) -> f64 {
    let n = points.len();
    let mut sum = 0.0;
    for k in 0..n {
        let a = points[k];
        let b = points[(k + 1) % n];
        sum += a.x * b.y - b.x * a.y;
    }
    0.5 * sum
}

/// Étape 0, `vertex_mean` : la moyenne des sommets, deux accumulateurs.
pub fn vertex_mean(points: &[Point]) -> Point {
    let mut x = 0.0;
    let mut y = 0.0;
    for p in points {
        x += p.x;
        y += p.y;
    }
    let n = points.len() as f64;
    Point::new(x / n, y / n)
}

/// Étape 0, `polygon_centroid` : la même formule que le corrigé, sans `fold`.
pub fn polygon_centroid(points: &[Point]) -> Point {
    let area = polygon_area(points);
    if area.abs() < 1e-16 {
        return vertex_mean(points);
    }
    let n = points.len();
    let mut sx = 0.0;
    let mut sy = 0.0;
    for k in 0..n {
        let a = points[k];
        let b = points[(k + 1) % n];
        let cross = a.x * b.y - b.x * a.y;
        sx += (a.x + b.x) * cross;
        sy += (a.y + b.y) * cross;
    }
    Point::new(sx / (6.0 * area), sy / (6.0 * area))
}

/// Étape 2, `fluid_neighbours` : quatre `if`, sans `Option`.
///
/// Le piège est la soustraction : `row - 1` sur un `usize` nul panique. On teste donc
/// `row > 0` **avant** de soustraire. Vers le bas et la droite, rien à craindre :
/// `is_fluid` répond déjà `false` hors de la grille.
pub fn fluid_neighbours(mask: &Mask, k: usize) -> Vec<usize> {
    let cols = mask.cols();
    let (row, col) = (k / cols, k % cols);
    let mut out = Vec::new();
    if row > 0 && mask.is_fluid(row - 1, col) {
        out.push((row - 1) * cols + col);
    }
    if mask.is_fluid(row + 1, col) {
        out.push((row + 1) * cols + col);
    }
    if col > 0 && mask.is_fluid(row, col - 1) {
        out.push(row * cols + col - 1);
    }
    if mask.is_fluid(row, col + 1) {
        out.push(row * cols + col + 1);
    }
    out
}

/// Étape 2, `explore` : un `Vec` pour pile, au lieu d'une `VecDeque` pour file.
///
/// Le parcours devient « en profondeur d'abord » au lieu de « en largeur » ; l'ordre de
/// visite change, pas l'ensemble des cellules atteintes, qui est tout ce qu'on demande.
/// Pas de récursion : sur un maillage raffiné, une composante compte des centaines de
/// milliers de cellules, et la pile d'appels déborderait.
pub fn explore(mask: &Mask, start: usize, seen: &mut [bool]) {
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(k) = stack.pop() {
        for neighbour in fluid_neighbours(mask, k) {
            if !seen[neighbour] {
                seen[neighbour] = true;
                stack.push(neighbour);
            }
        }
    }
}

/// Étape 2, le cœur de `check_connected` : le nombre de composantes fluides.
pub fn count_components(mask: &Mask) -> usize {
    let cols = mask.cols();
    let mut seen = vec![false; mask.rows() * cols];
    let mut components = 0;
    for k in 0..seen.len() {
        if mask.is_fluid(k / cols, k % cols) && !seen[k] {
            components += 1;
            explore(mask, k, &mut seen);
        }
    }
    components
}

/// Étape 5, `cell_transport` : un accumulateur et une boucle sur les faces.
pub fn cell_transport(mesh: &Mesh, id: CellId, face_flux: &[f64], diffusivity: f64) -> f64 {
    let mut total = 0.0;
    for &fid in mesh.cell_faces(id) {
        let face = mesh.face(fid);
        total += face_flux[fid.index()].abs() + 2.0 * diffusivity * face.length / face.distance;
    }
    total
}

/// Étape 5, `max_stable_dt` : le minimum courant dans une `Option`, sans `filter_map`
/// ni `reduce`.
pub fn max_stable_dt(mesh: &Mesh, face_flux: &[f64], diffusivity: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for (i, cell) in mesh.cells().iter().enumerate() {
        let transport = cell_transport(mesh, CellId(i as u32), face_flux, diffusivity);
        if transport > 0.0 {
            let candidate = cell.area / transport;
            best = match best {
                None => Some(candidate),
                Some(current) => Some(current.min(candidate)),
            };
        }
    }
    best
}

/// Étape 5, `step_euler` : une boucle sur les indices des deux tranches.
pub fn step_euler(dt: f64, c: &mut Field, work: &Field) {
    let c = c.as_mut_slice();
    let work = work.as_slice();
    for i in 0..c.len() {
        c[i] += dt * work[i];
    }
}

/// Étape 6, `mean_abs_error` : deux accumulateurs, une boucle sur les indices.
pub fn mean_abs_error(a: &Field, b: &Field, mesh: &Mesh) -> f64 {
    let (a, b) = (a.as_slice(), b.as_slice());
    let mut num = 0.0;
    let mut den = 0.0;
    for (i, cell) in mesh.cells().iter().enumerate() {
        num += (a[i] - b[i]).abs() * cell.area;
        den += cell.area;
    }
    num / den
}

#[cfg(test)]
mod tests {
    use super::*;
    use wind_tunnel::flux::Upwind;
    use wind_tunnel::geom::{self, Vec2};
    use wind_tunnel::mesh::FaceId;
    use wind_tunnel::solver::{Config, Solver, TimeScheme};
    use wind_tunnel::velocity::Uniform;

    fn shifted(p: Point, by: Vec2) -> Point {
        Point::new(p.x + by.x, p.y + by.y)
    }

    fn tunnel() -> Mask {
        Mask::from_file(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../domains/tunnel.dom"
        ))
        .expect("domains/tunnel.dom illisible")
    }

    #[test]
    fn geometry_matches_the_corrige_bit_for_bit() {
        let shapes: [&[Point]; 3] = [
            &[
                Point::new(0.0, 0.0),
                Point::new(3.0, 0.0),
                Point::new(0.0, 3.0),
            ],
            &[
                Point::new(1.0, 1.0),
                Point::new(2.5, 1.0),
                Point::new(2.5, 2.0),
                Point::new(1.0, 2.0),
            ],
            // dégénéré : l'aire est nulle, le centroïde retombe sur la moyenne des sommets
            &[
                Point::new(0.0, 0.0),
                Point::new(1.0, 1.0),
                Point::new(2.0, 2.0),
            ],
        ];
        for shape in shapes {
            let moved: Vec<Point> = shape
                .iter()
                .map(|&p| shifted(p, Vec2::new(0.3, -0.7)))
                .collect();
            for points in [shape, &moved[..]] {
                assert_eq!(polygon_area(points), geom::polygon_area(points));
                let (ours, theirs) = (polygon_centroid(points), geom::polygon_centroid(points));
                assert_eq!((ours.x, ours.y), (theirs.x, theirs.y));
            }
        }
    }

    #[test]
    fn the_connected_tunnel_has_one_component() {
        assert_eq!(count_components(&tunnel()), 1);
        assert_eq!(count_components(&tunnel().refine(3)), 1);
    }

    #[test]
    fn a_band_cut_by_the_obstacle_has_two_components() {
        // `Mask::parse` refuse un domaine coupé ; `columns` découpe sans vérifier.
        let mask = Mask::parse("...\n.#.\n...\n").unwrap().columns(1..2);
        assert_eq!(count_components(&mask), 2);
    }

    #[test]
    fn neighbours_stay_in_the_grid() {
        let mask = Mask::parse("..\n..\n").unwrap();
        assert_eq!(fluid_neighbours(&mask, 0), [2, 1]);
        assert_eq!(fluid_neighbours(&mask, 3), [1, 2]);
    }

    #[test]
    fn solver_variants_match_the_corrige_bit_for_bit() {
        let mesh = Mesh::from_mask(&tunnel(), 0.5).unwrap();
        let flow = Uniform {
            value: Vec2::new(1.0, 0.0),
        };
        let config = Config {
            diffusivity: 0.05,
            time_scheme: TimeScheme::Euler,
            ..Config::default()
        };
        let solver = Solver::new(&mesh, &flow, Upwind, config.clone()).unwrap();
        let face_flux: Vec<f64> = (0..mesh.n_faces())
            .map(|i| solver.face_flux(FaceId(i as u32)))
            .collect();
        assert_eq!(
            max_stable_dt(&mesh, &face_flux, config.diffusivity),
            Some(solver.max_stable_dt())
        );

        let start = wind_tunnel::app::initial_field(&mesh, 5);
        let (mut theirs, mut ours) = (start.clone(), start.clone());
        let mut work = Field::zeros(mesh.n_cells());
        solver.step(&mut theirs, &mut work);
        solver.residual(&ours, &mut work);
        step_euler(solver.dt(), &mut ours, &work);
        assert_eq!(ours, theirs);

        assert_eq!(
            mean_abs_error(&ours, &start, &mesh),
            ours.mean_abs_error(&start, &mesh)
        );
    }
}
