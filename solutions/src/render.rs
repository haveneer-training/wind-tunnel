//! Étape 10, « pour aller plus loin » : trois autres façons de paralléliser le rendu PNG.
//!
//! - [`render_spawn_arc`] — `std::thread::spawn` au lieu de `thread::scope` : la
//!   propriété réellement partagée, `Arc` et `Arc::try_unwrap` compris ;
//! - [`render_lock_free`] — plus de verrou : chaque thread renvoie ses pixels par le canal,
//!   une boucle séquentielle les recopie ;
//! - [`render_rayon`] — `rayon` et un `Mutex`, la version la plus courte.
//!
//! Toutes rendent **la même image** que [`render_sequential`], elle-même identique au
//! pixel près à `wind_tunnel::io::png::write_png` (les tests le vérifient). La géométrie
//! — projection, pixels d'une cellule, palette — vient du corrigé (`Raster`) : seule
//! l'organisation des threads change d'une variante à l'autre.
//!
//! `cargo run --release --manifest-path solutions/Cargo.toml --example render_timing`
//! chronomètre la version à verrou contre la version sans verrou.

use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use image::{Rgb, RgbImage};
use rayon::prelude::*;
use wind_tunnel::field::Field;
use wind_tunnel::geom::Point;
use wind_tunnel::io::png::{BACKGROUND, Raster};
use wind_tunnel::mesh::{CellId, Mesh};

/// Un pixel calculé, prêt à être écrit.
pub type Pixel = (u32, u32, Rgb<u8>);

/// Les tampons de travail d'un thread, réutilisés d'une cellule à l'autre.
#[derive(Default)]
struct Buffers {
    polygon: Vec<Point>,
    corners: Vec<(f64, f64)>,
    pixels: Vec<(u32, u32)>,
}

impl Buffers {
    /// Calcule les pixels de la cellule `i` dans `self.pixels`, et renvoie leur couleur.
    fn draw(&mut self, raster: &Raster, mesh: &Mesh, field: &Field, i: usize) -> Rgb<u8> {
        let id = CellId(i as u32);
        raster.cell_pixels(
            mesh,
            field[id],
            mesh.cell(id),
            &mut self.polygon,
            &mut self.corners,
            &mut self.pixels,
        )
    }

    /// Écrit dans l'image les pixels calculés par le dernier `draw`.
    fn paint(&self, image: &mut RgbImage, color: Rgb<u8>) {
        for &(x, y) in &self.pixels {
            image.put_pixel(x, y, color);
        }
    }
}

/// Une image vierge à la taille du rendu.
fn blank(raster: &Raster, width: u32) -> RgbImage {
    RgbImage::from_pixel(width, raster.height(), BACKGROUND)
}

/// Nombre de threads à lancer.
fn threads() -> usize {
    thread::available_parallelism().map_or(1, |n| n.get())
}

/// La référence : une seule boucle, pas de thread.
pub fn render_sequential(mesh: &Mesh, field: &Field, width: u32, range: (f64, f64)) -> RgbImage {
    let raster = Raster::new(mesh, width, range);
    let mut image = blank(&raster, width);
    let mut buffers = Buffers::default();
    for i in 0..mesh.n_cells() {
        let color = buffers.draw(&raster, mesh, field, i);
        buffers.paint(&mut image, color);
    }
    image
}

/// `std::thread::spawn` et `Arc<Mutex<…>>` : la propriété réellement partagée.
///
/// Un thread lancé par `spawn` peut survivre à la fonction qui l'a lancé : le compilateur
/// exige donc qu'il ne capture rien d'emprunté (`'static`). Le maillage, le champ et la
/// projection passent dans des `Arc` — un `Arc::clone` par thread, qui ne copie que le
/// pointeur —, l'image dans un `Arc<Mutex<…>>`. Une fois tous les threads joints, il ne
/// reste qu'un seul `Arc` sur l'image, et `Arc::try_unwrap` la récupère.
///
/// Chaque thread ne peut plus emprunter une tranche `&[Cell]` du maillage : il reçoit un
/// intervalle d'indices, et relit le maillage à travers son `Arc`.
pub fn render_spawn_arc(
    mesh: Arc<Mesh>,
    field: Arc<Field>,
    width: u32,
    range: (f64, f64),
) -> RgbImage {
    let raster = Arc::new(Raster::new(&mesh, width, range));
    let image = Arc::new(Mutex::new(blank(&raster, width)));
    let n = mesh.n_cells();
    let chunk = n.div_ceil(threads()).max(1);

    let mut handles = Vec::new();
    for start in (0..n).step_by(chunk) {
        let (mesh, field, raster, image) = (
            Arc::clone(&mesh),
            Arc::clone(&field),
            Arc::clone(&raster),
            Arc::clone(&image),
        );
        handles.push(thread::spawn(move || {
            let mut buffers = Buffers::default();
            for i in start..(start + chunk).min(n) {
                let color = buffers.draw(&raster, &mesh, &field, i);
                buffers.paint(&mut image.lock().unwrap(), color);
            }
        }));
    }
    for handle in handles {
        handle.join().expect("un thread de rendu a paniqué");
    }
    // Tous les clones ont été détruits avec leur thread : il n'en reste qu'un.
    Arc::try_unwrap(image)
        .unwrap_or_else(|_| panic!("l'image est encore partagée"))
        .into_inner()
        .unwrap()
}

/// Sans verrou : les threads calculent, le canal transporte, un seul écrit.
///
/// Chaque thread renvoie la liste de ses pixels au lieu de les écrire ; après le
/// `thread::scope`, une boucle séquentielle les recopie dans l'image. Plus de `Mutex`,
/// donc plus d'attente — mais tous les pixels transitent par la mémoire une fois de plus.
/// Qui gagne dépend du rapport entre le coût du calcul d'une cellule et celui de sa
/// recopie : voir l'exemple `render_timing`.
pub fn render_lock_free(mesh: &Mesh, field: &Field, width: u32, range: (f64, f64)) -> RgbImage {
    let raster = Raster::new(mesh, width, range);
    let chunk = mesh.n_cells().div_ceil(threads()).max(1);
    let (tx, rx) = mpsc::channel::<Vec<Pixel>>();
    thread::scope(|scope| {
        for (index, cells) in mesh.cells().chunks(chunk).enumerate() {
            let tx = tx.clone();
            let raster = &raster;
            scope.spawn(move || {
                let mut buffers = Buffers::default();
                let mut out: Vec<Pixel> = Vec::new();
                for local in 0..cells.len() {
                    let color = buffers.draw(raster, mesh, field, index * chunk + local);
                    out.extend(buffers.pixels.iter().map(|&(x, y)| (x, y, color)));
                }
                tx.send(out)
                    .expect("le récepteur vit jusqu'à la fin du scope");
            });
        }
    });
    // Le `tx` d'origine vit encore : `rx.iter()` attendrait pour toujours. `try_iter`
    // ne lit que ce qui est déjà arrivé — tout, puisque les threads sont finis.
    drop(tx);
    let mut image = blank(&raster, width);
    for pixels in rx.try_iter() {
        for (x, y, color) in pixels {
            image.put_pixel(x, y, color);
        }
    }
    image
}

/// `rayon` et un `Mutex` : la version la plus courte.
///
/// `rayon` découpe le travail, lance et joint les threads ; il ne reste à écrire que le
/// corps d'une cellule. C'est ce que cache l'étape 9, et que l'étape 10 fait à la main.
pub fn render_rayon(mesh: &Mesh, field: &Field, width: u32, range: (f64, f64)) -> RgbImage {
    let raster = Raster::new(mesh, width, range);
    let image = Mutex::new(blank(&raster, width));
    (0..mesh.n_cells())
        .into_par_iter()
        .for_each_init(Buffers::default, |buffers, i| {
            let color = buffers.draw(&raster, mesh, field, i);
            buffers.paint(&mut image.lock().unwrap(), color);
        });
    image.into_inner().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wind_tunnel::mask::Mask;

    fn case() -> (Mesh, Field) {
        let mask = Mask::from_file(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../domains/tunnel.dom"
        ))
        .expect("domains/tunnel.dom illisible");
        let mesh = Mesh::from_mask(&mask, 0.5).unwrap();
        let field = wind_tunnel::app::initial_field(&mesh, 7);
        (mesh, field)
    }

    #[test]
    fn the_reference_matches_write_png_pixel_for_pixel() {
        let (mesh, field) = case();
        let path =
            std::env::temp_dir().join(format!("solutions-render-{}.png", std::process::id()));
        wind_tunnel::io::png::write_png(&path, &mesh, &field, 300, (0.0, 1.0)).unwrap();
        let theirs = image::open(&path).unwrap().to_rgb8();
        let _ = std::fs::remove_file(&path);
        assert_eq!(render_sequential(&mesh, &field, 300, (0.0, 1.0)), theirs);
    }

    #[test]
    fn every_variant_renders_the_same_image() {
        let (mesh, field) = case();
        let reference = render_sequential(&mesh, &field, 300, (0.0, 1.0));
        assert_eq!(render_lock_free(&mesh, &field, 300, (0.0, 1.0)), reference);
        assert_eq!(render_rayon(&mesh, &field, 300, (0.0, 1.0)), reference);
        let shared = render_spawn_arc(Arc::new(mesh), Arc::new(field), 300, (0.0, 1.0));
        assert_eq!(shared, reference);
    }
}
