//! Rendu d'un champ en image PNG.
//!
//! Le tracé est délibérément naïf — pour chaque cellule, on teste les pixels de sa
//! boîte englobante — mais il ne suppose rien de la structure du maillage : il
//! fonctionnera tel quel sur un maillage lu depuis un fichier.

#[cfg(feature = "step10")]
use std::sync::{Mutex, mpsc};
#[cfg(feature = "step10")]
use std::thread;

use image::{ImageError, Rgb, RgbImage};

use crate::field::Field;
use crate::geom::Point;
use crate::mesh::{Cell, CellId, Mesh};

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

/// Projection et palette d'un rendu, partagées entre les chemins séquentiel et parallèle.
struct Raster {
    xmin: f64,
    ymax: f64,
    scale: f64,
    width: u32,
    height: u32,
    lo: f64,
    span: f64,
}

impl Raster {
    fn new(mesh: &Mesh, width: u32, range: (f64, f64)) -> Raster {
        let (xmin, ymin, xmax, ymax) = mesh.bounds();
        let scale = width as f64 / (xmax - xmin);
        let height = (((ymax - ymin) * scale).round() as u32).max(1);
        let (lo, hi) = range;
        let span = if (hi - lo).abs() < f64::EPSILON {
            1.0
        } else {
            hi - lo
        };
        Raster {
            xmin,
            ymax,
            scale,
            width,
            height,
            lo,
            span,
        }
    }

    fn cell_pixels(
        &self,
        mesh: &Mesh,
        value: f64,
        cell: &Cell,
        polygon: &mut Vec<Point>,
        corners: &mut Vec<(f64, f64)>,
        pixels: &mut Vec<(u32, u32)>,
    ) -> Rgb<u8> {
        polygon.clear();
        polygon.extend(
            cell.kind
                .vertices()
                .iter()
                .map(|vertex| mesh.vertices()[vertex.index()]),
        );
        let color = colormap((value - self.lo) / self.span);
        let to_pixel = |point: Point| {
            (
                (point.x - self.xmin) * self.scale,
                (self.ymax - point.y) * self.scale,
            )
        };
        corners.clear();
        corners.extend(polygon.iter().map(|&point| to_pixel(point)));
        let px0 = corners
            .iter()
            .map(|corner| corner.0)
            .fold(f64::MAX, f64::min)
            .floor() as i64;
        let px1 = corners
            .iter()
            .map(|corner| corner.0)
            .fold(f64::MIN, f64::max)
            .ceil() as i64;
        let py0 = corners
            .iter()
            .map(|corner| corner.1)
            .fold(f64::MAX, f64::min)
            .floor() as i64;
        let py1 = corners
            .iter()
            .map(|corner| corner.1)
            .fold(f64::MIN, f64::max)
            .ceil() as i64;
        pixels.clear();
        for py in py0.max(0)..=py1.min(self.height as i64 - 1) {
            for px in px0.max(0)..=px1.min(self.width as i64 - 1) {
                let sample = Point::new(
                    self.xmin + (px as f64 + 0.5) / self.scale,
                    self.ymax - (py as f64 + 0.5) / self.scale,
                );
                if contains(polygon, sample) {
                    pixels.push((px as u32, py as u32));
                }
            }
        }
        color
    }
}

#[cfg(feature = "step10")]
struct RenderBuffers {
    polygon: Vec<Point>,
    corners: Vec<(f64, f64)>,
    pixels: Vec<(u32, u32)>,
}

#[cfg(feature = "step10")]
impl RenderBuffers {
    fn new() -> RenderBuffers {
        RenderBuffers {
            polygon: Vec::with_capacity(4),
            corners: Vec::with_capacity(4),
            pixels: Vec::new(),
        }
    }
}

/// Le point est-il dans le polygone ? (lancer de rayon)
#[cfg_attr(not(feature = "step3"), allow(unused_variables))] // trou étape 3
#[cfg_attr(not(feature = "step11"), allow(dead_code))] // collatéral du trou étape 10
fn contains(polygon: &[Point], p: Point) -> bool {
    // TODO-STEP:3 (pour aller plus loin) Inverser le résultat à chaque arête qui
    // traverse le rayon horizontal partant de `p`.
    // HINT : les arêtes sont `(polygon[k], polygon[(k + 1) % n])` ; `inside = !inside` à
    // chaque arête pour laquelle `crosses` répond vrai.
    // SOLUTION-BEGIN
    let mut inside = false;
    let n = polygon.len();
    for (k, a) in polygon.iter().enumerate() {
        let b = &polygon[(k + 1) % n];
        if crosses(*a, *b, p) {
            inside = !inside;
        }
    }
    inside
    // SOLUTION-END
}

/// Le segment `(a, b)` traverse-t-il le rayon horizontal lancé depuis `p` ?
#[cfg_attr(not(feature = "step4"), allow(dead_code))] // appelée depuis le trou de contains
fn crosses(a: Point, b: Point, p: Point) -> bool {
    // TODO-STEP:3 Tester le changement de côté vertical, puis comparer l'abscisse
    // du point d'intersection avec celle de `p`.
    // HINT : si `(a.y > p.y) == (b.y > p.y)`, l'arête ne coupe pas l'horizontale de `p`
    // → `false`. Sinon l'intersection est en
    //     x = a.x + (p.y − a.y) / (b.y − a.y) · (b.x − a.x)
    // et l'arête compte si `p.x < x`.
    // SOLUTION-BEGIN
    if (a.y > p.y) == (b.y > p.y) {
        return false;
    }
    let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
    p.x < x
    // SOLUTION-END
}

/// Rend un champ en PNG, `width` pixels de large.
///
/// `range` fixe les bornes de la palette ; passer les extrema du champ donne un rendu
/// à contraste maximal mais qui « respire » d'une image à l'autre, ce qui est trompeur
/// dans une animation.
#[cfg(feature = "step10")]
pub fn write_png(
    path: impl AsRef<std::path::Path>,
    mesh: &Mesh,
    field: &Field,
    width: u32,
    range: (f64, f64),
) -> Result<(), ImageError> {
    let raster = Raster::new(mesh, width, range);
    let img = RgbImage::from_pixel(width, raster.height, BACKGROUND);
    let image = Mutex::new(img);
    let n_threads = thread::available_parallelism().map_or(1, |n| n.get());
    let chunk_size = chunk_size(mesh.n_cells(), n_threads);
    let drawn = render_all(mesh, field, &raster, &image, chunk_size);
    debug_assert_eq!(
        drawn,
        mesh.n_cells(),
        "toutes les cellules doivent être rendues"
    );
    let img = image.into_inner().unwrap();
    img.save(path)
}

/// Rendu d'une tranche par un seul thread, avec ses tampons de travail privés.
#[cfg(feature = "step10")]
#[cfg_attr(not(feature = "step11"), allow(dead_code))] // appelée depuis le trou de l'étape 10
fn render_chunk(
    cells: &[Cell],
    base: usize,
    mesh: &Mesh,
    field: &Field,
    raster: &Raster,
    image: &Mutex<RgbImage>,
    tx: mpsc::Sender<usize>,
) {
    // TODO-STEP:10 Réutiliser les tampons du thread, rendre chaque cellule, puis
    // envoyer le nombre de cellules traitées.
    // HINT : `RenderBuffers::new()` une fois pour la tranche ; l'identifiant global d'une
    // cellule est `CellId((base + local) as u32)` ; `tx.send(n)` rend un `Result`.
    // SOLUTION-BEGIN
    let mut buffers = RenderBuffers::new();
    for (local, cell) in cells.iter().enumerate() {
        let id = CellId((base + local) as u32);
        render_cell(cell, id, mesh, field, raster, image, &mut buffers);
    }
    tx.send(cells.len())
        .expect("le suivi ne doit pas fermer trop tôt");
    // SOLUTION-END
}

/// Calcule les pixels d'une cellule puis les écrit dans l'image protégée.
#[cfg(feature = "step10")]
#[cfg_attr(not(feature = "step11"), allow(dead_code))] // appelée depuis render_chunk
fn render_cell(
    cell: &Cell,
    id: CellId,
    mesh: &Mesh,
    field: &Field,
    raster: &Raster,
    image: &Mutex<RgbImage>,
    buffers: &mut RenderBuffers,
) {
    // TODO-STEP:10 Utiliser `Raster` pour obtenir les pixels de la cellule, puis les
    // écrire avec `paint` en réutilisant les tampons.
    // HINT : `cell_pixels` veut trois `&mut` à la fois : `&mut buffers.polygon`,
    // `&mut buffers.corners`, `&mut buffers.pixels`. Le compilateur l'accepte car ce sont des
    // **champs distincts** (emprunts disjoints) — il refuserait la même chose à travers une
    // méthode `&mut self` de `RenderBuffers`. Puis `paint(image, &buffers.pixels, color)`.
    // SOLUTION-BEGIN
    let color = raster.cell_pixels(
        mesh,
        field[id],
        cell,
        &mut buffers.polygon,
        &mut buffers.corners,
        &mut buffers.pixels,
    );
    paint(image, &buffers.pixels, color);
    // SOLUTION-END
}

/// Taille de tranche équilibrée, non nulle même pour une entrée vide.
#[cfg(feature = "step10")]
#[cfg_attr(not(feature = "step11"), allow(dead_code))] // appelée depuis le rendu
fn chunk_size(n: usize, threads: usize) -> usize {
    // TODO-STEP:10 Répartir les cellules aussi également que possible et éviter
    // une taille nulle, même si `threads` vaut zéro.
    // HINT : `usize::div_ceil` arrondit au-dessus ; `max(1)` évite le zéro, avant la
    // division comme après.
    // SOLUTION-BEGIN
    n.div_ceil(threads.max(1)).max(1)
    // SOLUTION-END
}

/// Écrit une couleur à tous les pixels d'une cellule sous un seul verrou.
#[cfg(feature = "step10")]
#[cfg_attr(not(feature = "step11"), allow(dead_code))] // appelée depuis render_chunk
fn paint(image: &Mutex<RgbImage>, pixels: &[(u32, u32)], color: Rgb<u8>) {
    // TODO-STEP:10 Verrouiller l'image une fois et écrire chaque pixel calculé.
    // HINT : `image.lock().unwrap()` rend un garde : le verrou tient tant qu'il vit. Le
    // prendre une seule fois avant la boucle, puis `put_pixel(x, y, color)`.
    // SOLUTION-BEGIN
    let mut image = image.lock().unwrap();
    for &(x, y) in pixels {
        image.put_pixel(x, y, color);
    }
    // SOLUTION-END
}

/// Orchestration des tranches dans des threads scoped et collecte du compte rendu.
#[cfg(feature = "step10")]
#[cfg_attr(not(feature = "step11"), allow(dead_code))] // appelée depuis write_png
fn render_all(
    mesh: &Mesh,
    field: &Field,
    raster: &Raster,
    image: &Mutex<RgbImage>,
    chunk: usize,
) -> usize {
    // TODO-STEP:10 Lancer un thread scoped par tranche, récupérer les comptes via
    // le canal et renvoyer leur somme.
    // HINT : `let (tx, rx) = mpsc::channel::<usize>()` ; dans `thread::scope(|scope| …)`,
    // une tranche par élément de `mesh.cells().chunks(chunk)`, lancée par
    // `scope.spawn(move || render_chunk(…))` avec son propre clone de `tx`.
    // Piège : `rx.iter()` ne s'arrête que quand *tous* les `Sender` sont détruits —
    // `drop(tx)` sur l'original avant de lire, sinon l'appel attend pour toujours.
    // SOLUTION-BEGIN
    let (tx, rx) = mpsc::channel::<usize>();
    thread::scope(|scope| {
        for (index, cells) in mesh.cells().chunks(chunk).enumerate() {
            let tx = tx.clone();
            let base = index * chunk;
            scope.spawn(move || render_chunk(cells, base, mesh, field, raster, image, tx));
        }
        drop(tx);
        rx.iter().sum()
    })
    // SOLUTION-END
}

/// Rend un champ en PNG, `width` pixels de large — version séquentielle, avant l'étape
/// 10.
///
/// `range` fixe les bornes de la palette ; passer les extrema du champ donne un rendu
/// à contraste maximal mais qui « respire » d'une image à l'autre, ce qui est trompeur
/// dans une animation.
#[cfg(not(feature = "step10"))]
pub fn write_png(
    path: impl AsRef<std::path::Path>,
    mesh: &Mesh,
    field: &Field,
    width: u32,
    range: (f64, f64),
) -> Result<(), ImageError> {
    let raster = Raster::new(mesh, width, range);
    let mut img = RgbImage::from_pixel(width, raster.height, BACKGROUND);
    let mut polygon = Vec::with_capacity(4);
    let mut corners = Vec::with_capacity(4);
    let mut pixels = Vec::new();
    for (i, cell) in mesh.cells().iter().enumerate() {
        let id = CellId(i as u32);
        let color = raster.cell_pixels(
            mesh,
            field[id],
            cell,
            &mut polygon,
            &mut corners,
            &mut pixels,
        );
        for &(x, y) in &pixels {
            img.put_pixel(x, y, color);
        }
    }

    img.save(path)
}

#[cfg(all(test, feature = "step3"))]
mod contains_tests {
    use super::*;

    #[test]
    fn crosses_only_when_the_ray_intersects_the_segment() {
        assert!(crosses(
            Point::new(1.0, -1.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 0.0)
        ));
        assert!(!crosses(
            Point::new(-1.0, -1.0),
            Point::new(-1.0, 1.0),
            Point::new(0.0, 0.0)
        ));
    }

    #[test]
    fn contains_distinguishes_inside_and_outside_points() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(0.0, 1.0),
        ];
        assert!(contains(&square, Point::new(0.5, 0.5)));
        assert!(!contains(&square, Point::new(2.0, 0.5)));
    }
}

#[cfg(all(test, feature = "step10"))]
mod parallel_tests {
    use super::*;
    use crate::mask::Mask;

    fn setup() -> (Mesh, Field, Raster) {
        let mesh = Mesh::from_mask(&Mask::parse("..\n").unwrap(), 1.0).unwrap();
        let field = Field::filled(mesh.n_cells(), 0.5);
        let raster = Raster::new(&mesh, 16, (0.0, 1.0));
        (mesh, field, raster)
    }

    #[test]
    fn chunk_size_handles_empty_work_and_zero_threads() {
        assert_eq!(chunk_size(0, 4), 1);
        assert_eq!(chunk_size(7, 0), 7);
        assert_eq!(chunk_size(10, 3), 4);
    }

    #[test]
    fn paint_writes_only_the_requested_pixels() {
        let image = Mutex::new(RgbImage::from_pixel(2, 1, BACKGROUND));
        let color = Rgb([1, 2, 3]);
        paint(&image, &[(1, 0)], color);
        let image = image.lock().unwrap();
        assert_eq!(*image.get_pixel(0, 0), BACKGROUND);
        assert_eq!(*image.get_pixel(1, 0), color);
    }

    #[test]
    fn render_chunk_reports_its_cell_count() {
        let (mesh, field, raster) = setup();
        let image = Mutex::new(RgbImage::from_pixel(
            raster.width,
            raster.height,
            BACKGROUND,
        ));
        let (tx, rx) = mpsc::channel();
        render_chunk(mesh.cells(), 0, &mesh, &field, &raster, &image, tx);
        assert_eq!(rx.recv().unwrap(), mesh.n_cells());
    }

    #[test]
    fn render_cell_populates_pixels_for_one_cell() {
        let (mesh, field, raster) = setup();
        let image = Mutex::new(RgbImage::from_pixel(
            raster.width,
            raster.height,
            BACKGROUND,
        ));
        let mut buffers = RenderBuffers::new();
        render_cell(
            &mesh.cells()[0],
            CellId(0),
            &mesh,
            &field,
            &raster,
            &image,
            &mut buffers,
        );
        assert!(!buffers.pixels.is_empty());
    }

    #[test]
    fn render_all_collects_all_cell_counts() {
        let (mesh, field, raster) = setup();
        let image = Mutex::new(RgbImage::from_pixel(
            raster.width,
            raster.height,
            BACKGROUND,
        ));
        assert_eq!(
            render_all(&mesh, &field, &raster, &image, 1),
            mesh.n_cells()
        );
    }
}
