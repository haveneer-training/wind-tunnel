//! Rendu d'un champ en image PNG.
//!
//! Le tracé est délibérément naïf — pour chaque cellule, on teste les pixels de sa
//! boîte englobante — mais il ne suppose rien de la structure du maillage : il
//! fonctionnera tel quel sur un maillage lu depuis un fichier.

use image::{ImageError, Rgb, RgbImage};

use crate::field::Field;
use crate::geom::Point;
use crate::mesh::{CellId, Mesh};

/// Couleur du fond, c'est-à-dire du solide.
///
/// Un gris neutre, volontairement étranger à la palette : ce qui n'est pas une donnée
/// ne doit pas pouvoir se confondre avec une valeur faible.
const BACKGROUND: Rgb<u8> = Rgb([88, 88, 96]);

/// Points d'ancrage de la palette (type « inferno »).
const PALETTE: [[f64; 3]; 5] = [
    [0.0, 0.0, 4.0],
    [85.0, 15.0, 109.0],
    [187.0, 55.0, 84.0],
    [249.0, 142.0, 9.0],
    [252.0, 255.0, 164.0],
];

/// Couleur associée à une valeur normalisée dans `[0, 1]`.
pub fn colormap(t: f64) -> Rgb<u8> {
    let t = t.clamp(0.0, 1.0) * (PALETTE.len() - 1) as f64;
    let i = (t.floor() as usize).min(PALETTE.len() - 2);
    let f = t - i as f64;
    let (a, b) = (PALETTE[i], PALETTE[i + 1]);
    Rgb([
        (a[0] + f * (b[0] - a[0])).round() as u8,
        (a[1] + f * (b[1] - a[1])).round() as u8,
        (a[2] + f * (b[2] - a[2])).round() as u8,
    ])
}

/// Le point est-il dans le polygone ? (lancer de rayon)
fn contains(polygon: &[Point], p: Point) -> bool {
    // TODO-STEP:3 (pour aller plus loin) Lancer un rayon horizontal depuis `p` et
    // compter les arêtes traversées : un nombre impair signifie « à l'intérieur »
    // SOLUTION-BEGIN
    let mut inside = false;
    let n = polygon.len();
    for (k, a) in polygon.iter().enumerate() {
        let b = &polygon[(k + 1) % n];
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
    // SOLUTION-END
}

/// Rend un champ en PNG, `width` pixels de large.
///
/// `range` fixe les bornes de la palette ; passer les extrema du champ donne un rendu
/// à contraste maximal mais qui « respire » d'une image à l'autre, ce qui est trompeur
/// dans une animation.
pub fn write_png(
    path: impl AsRef<std::path::Path>,
    mesh: &Mesh,
    field: &Field,
    width: u32,
    range: (f64, f64),
) -> Result<(), ImageError> {
    let (xmin, ymin, xmax, ymax) = mesh.bounds();
    let scale = width as f64 / (xmax - xmin);
    let height = (((ymax - ymin) * scale).round() as u32).max(1);
    let mut img = RgbImage::from_pixel(width, height, BACKGROUND);

    let (lo, hi) = range;
    let span = if (hi - lo).abs() < f64::EPSILON {
        1.0
    } else {
        hi - lo
    };

    let to_pixel = |p: Point| ((p.x - xmin) * scale, (ymax - p.y) * scale);

    for (i, cell) in mesh.cells().iter().enumerate() {
        let polygon: Vec<Point> = cell
            .kind
            .vertices()
            .iter()
            .map(|v| mesh.vertices()[v.index()])
            .collect();
        let color = colormap((field[CellId(i as u32)] - lo) / span);

        let corners: Vec<(f64, f64)> = polygon.iter().map(|&p| to_pixel(p)).collect();
        let px0 = corners.iter().map(|c| c.0).fold(f64::MAX, f64::min).floor() as i64;
        let px1 = corners.iter().map(|c| c.0).fold(f64::MIN, f64::max).ceil() as i64;
        let py0 = corners.iter().map(|c| c.1).fold(f64::MAX, f64::min).floor() as i64;
        let py1 = corners.iter().map(|c| c.1).fold(f64::MIN, f64::max).ceil() as i64;

        for py in py0.max(0)..=py1.min(height as i64 - 1) {
            for px in px0.max(0)..=px1.min(width as i64 - 1) {
                let sample = Point::new(
                    xmin + (px as f64 + 0.5) / scale,
                    ymax - (py as f64 + 0.5) / scale,
                );
                if contains(&polygon, sample) {
                    img.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }

    img.save(path)
}
