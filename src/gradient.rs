//! Reconstruction de gradient par moindres carrés, et limiteur associé.
//!
//! Un schéma d'ordre 1 comme `Upwind` retient la valeur de la cellule amont telle
//! quelle sur toute la face : c'est ce qui la rend diffusive (voir l'étape 6). Un
//! schéma d'ordre 2 l'extrapole linéairement jusqu'à la face, à partir d'un gradient
//! `(∂c/∂x, ∂c/∂y)` reconstruit par cellule — encore faut-il ce gradient, sur un
//! maillage non structuré où les voisins ne sont ni alignés sur des axes, ni
//! équidistants, ni même en nombre fixe (un triangle chanfreiné en a moins qu'un
//! quadrangle).
//!
//! La reconstruction extrapole les valeurs *avant* de les limiter, et rien ne garantit
//! alors qu'une face reste entre les valeurs des cellules qui l'entourent : c'est
//! exactement le défaut de `Centered`, qui fait déborder le champ de `[0, 1]` (étape
//! 6). Le limiteur de Barth–Jespersen corrige ça après coup, cellule par cellule.

#[cfg(feature = "step9")]
use rayon::prelude::*;

use crate::field::Field;
use crate::geom::Vec2;
use crate::mesh::{CellId, Mesh};

/// Cellule voisine d'une face donnée, vue depuis `id` — quel que soit le côté sur
/// lequel `id` se trouve.
fn neighbour_across(id: CellId, face: &crate::mesh::Face) -> Option<CellId> {
    if face.left == id {
        face.neighbor()
    } else {
        Some(face.left)
    }
}

/// Gradient `(∂c/∂x, ∂c/∂y)` d'une cellule par moindres carrés pondérés sur ses voisins
/// intérieurs : le vecteur qui explique le mieux, au sens des moindres carrés, l'écart
/// de `c` observé vers chaque centroïde voisin.
///
/// Pondération par l'inverse du carré de la distance — un voisin proche compte plus
/// qu'un voisin lointain. Avec moins de deux directions indépendantes (cellule sans
/// voisin intérieur, ou tous alignés), le système est singulier et la fonction renvoie
/// le vecteur nul plutôt que de diviser par zéro : rien à reconstruire.
fn least_squares_gradient(mesh: &Mesh, c: &Field, id: CellId) -> Vec2 {
    let ci = c[id];
    let xi = mesh.cell(id).centroid;

    // TODO-STEP:7 Assembler le système normal 2×2 pondéré sur les voisins intérieurs
    // — Σ w·dx·dxᵀ, Σ w·dx·dc — et le résoudre pour (∂c/∂x, ∂c/∂y). Vecteur nul si le
    // système est singulier.
    // SOLUTION-BEGIN
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    let (mut sxc, mut syc) = (0.0, 0.0);

    for &fid in mesh.cell_faces(id) {
        let face = mesh.face(fid);
        let Some(neighbour) = neighbour_across(id, face) else {
            continue;
        };
        let dx = mesh.cell(neighbour).centroid - xi;
        let dc = c[neighbour] - ci;
        let w = 1.0 / dx.dot(dx);

        sxx += w * dx.x * dx.x;
        sxy += w * dx.x * dx.y;
        syy += w * dx.y * dx.y;
        sxc += w * dx.x * dc;
        syc += w * dx.y * dc;
    }

    let det = sxx * syy - sxy * sxy;
    if det.abs() < 1e-300 {
        return Vec2::ZERO;
    }
    Vec2::new((syy * sxc - sxy * syc) / det, (sxx * syc - sxy * sxc) / det)
    // SOLUTION-END
}

/// Facteur limiteur de Barth–Jespersen d'une cellule : le plus grand `φ ∈ [0, 1]` tel
/// que la reconstruction `cᵢ + φ·∇cᵢ·(x_face − xᵢ)` reste, sur chacune de ses faces,
/// entre le minimum et le maximum des valeurs des cellules voisines.
///
/// `φ = 1` : le gradient passe tel quel, rien à corriger. `φ = 0` : la cellule est un
/// extremum local (un pic, un creux) et toute extrapolation le ferait déborder — on
/// retombe alors exactement sur l'ordre 1.
fn barth_jespersen(mesh: &Mesh, c: &Field, gradient: Vec2, id: CellId) -> f64 {
    let ci = c[id];
    let xi = mesh.cell(id).centroid;

    let (mut c_min, mut c_max) = (ci, ci);
    for &fid in mesh.cell_faces(id) {
        let face = mesh.face(fid);
        if let Some(neighbour) = neighbour_across(id, face) {
            c_min = c_min.min(c[neighbour]);
            c_max = c_max.max(c[neighbour]);
        }
    }

    let mut phi = 1.0f64;
    for &fid in mesh.cell_faces(id) {
        let face = mesh.face(fid);
        let delta = gradient.dot(face.midpoint - xi);
        if delta > 1e-300 {
            phi = phi.min((c_max - ci) / delta);
        } else if delta < -1e-300 {
            phi = phi.min((c_min - ci) / delta);
        }
    }
    phi.clamp(0.0, 1.0)
}

/// Gradient limité de chaque cellule du maillage, prêt à nourrir un schéma d'ordre 2.
///
/// Recalculé à chaque appel plutôt que mis en cache : l'écoulement est stationnaire,
/// mais `c` change à chaque pas de temps, donc le gradient aussi. `out` doit avoir une
/// entrée par cellule ; l'appelant le fournit pour ne rien allouer ici.
#[cfg(feature = "step9")]
pub fn limited_gradients(mesh: &Mesh, c: &Field, out: &mut [Vec2]) {
    // TODO-STEP:9 Paralléliser avec rayon : chaque cellule ne lit que `mesh` et `c`
    // (partagés, en lecture seule) et n'écrit que sa propre case de `out`.
    // SOLUTION-BEGIN
    out.par_iter_mut()
        .enumerate()
        .take(mesh.n_cells())
        .for_each(|(i, slot)| {
            let id = CellId(i as u32);
            let gradient = least_squares_gradient(mesh, c, id);
            let phi = barth_jespersen(mesh, c, gradient, id);
            *slot = gradient * phi;
        });
    // SOLUTION-END
}

/// Gradient limité de chaque cellule du maillage, prêt à nourrir un schéma d'ordre 2 —
/// version séquentielle, avant l'étape 9.
///
/// Recalculé à chaque appel plutôt que mis en cache : l'écoulement est stationnaire,
/// mais `c` change à chaque pas de temps, donc le gradient aussi. `out` doit avoir une
/// entrée par cellule ; l'appelant le fournit pour ne rien allouer ici.
#[cfg(not(feature = "step9"))]
pub fn limited_gradients(mesh: &Mesh, c: &Field, out: &mut [Vec2]) {
    for (i, slot) in out.iter_mut().enumerate().take(mesh.n_cells()) {
        let id = CellId(i as u32);
        let gradient = least_squares_gradient(mesh, c, id);
        let phi = barth_jespersen(mesh, c, gradient, id);
        *slot = gradient * phi;
    }
}

#[cfg(all(test, feature = "step7"))]
mod tests {
    use super::*;
    use crate::mask::Mask;

    #[test]
    fn gradient_of_a_linear_field_is_exact() {
        // moindres carrés : la reconstruction est exacte pour un champ affine, quelle
        // que soit l'irrégularité du maillage — c'est même sa raison d'être.
        let mesh = Mesh::from_mask(
            &Mask::parse("......\n......\n......\n......\n").unwrap(),
            1.0,
        )
        .unwrap();
        let (a, b) = (3.0, -1.5);
        let c = Field::from_fn(&mesh, |id| {
            let p = mesh.cell(id).centroid;
            2.0 + a * p.x + b * p.y
        });

        for i in 0..mesh.n_cells() {
            let g = least_squares_gradient(&mesh, &c, CellId(i as u32));
            assert!((g.x - a).abs() < 1e-9, "cellule {i} : ∂c/∂x = {}", g.x);
            assert!((g.y - b).abs() < 1e-9, "cellule {i} : ∂c/∂y = {}", g.y);
        }
    }

    #[test]
    fn gradient_of_a_uniform_field_is_zero() {
        let mesh = Mesh::from_mask(&Mask::parse("...\n...\n...\n").unwrap(), 1.0).unwrap();
        let c = Field::filled(mesh.n_cells(), 4.0);
        let mut gradients = vec![Vec2::ZERO; mesh.n_cells()];
        limited_gradients(&mesh, &c, &mut gradients);
        assert!(gradients.iter().all(|g| g.norm() < 1e-12));
    }

    #[test]
    fn limiter_keeps_the_reconstruction_within_neighbour_bounds() {
        // un pic isolé au milieu d'un champ plat : sans limiteur, l'extrapolation
        // déborderait largement de [0, 5] sur les faces voisines.
        let mesh = Mesh::from_mask(
            &Mask::parse("......\n......\n......\n......\n").unwrap(),
            1.0,
        )
        .unwrap();
        let spike = CellId(14);
        let c = Field::from_fn(&mesh, |id| if id == spike { 5.0 } else { 0.0 });

        let mut gradients = vec![Vec2::ZERO; mesh.n_cells()];
        limited_gradients(&mesh, &c, &mut gradients);

        let ci = c[spike];
        let xi = mesh.cell(spike).centroid;
        let (mut c_min, mut c_max) = (ci, ci);
        for &fid in mesh.cell_faces(spike) {
            if let Some(n) = neighbour_across(spike, mesh.face(fid)) {
                c_min = c_min.min(c[n]);
                c_max = c_max.max(c[n]);
            }
        }
        for &fid in mesh.cell_faces(spike) {
            let face = mesh.face(fid);
            let reconstructed = ci + gradients[spike.index()].dot(face.midpoint - xi);
            assert!(
                reconstructed >= c_min - 1e-9 && reconstructed <= c_max + 1e-9,
                "face {fid:?} : reconstruit {reconstructed}, borne [{c_min}, {c_max}]"
            );
        }
    }
}
