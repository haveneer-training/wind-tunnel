#![cfg(feature = "step2")]

//! Invariants du maillage, vérifiés sur le domaine réellement livré.
//!
//! Ce sont des propriétés géométriques exactes : elles ne dépendent ni du schéma, ni du
//! pas de temps, ni de la machine. Si l'une d'elles casse, le maillage est faux, et tout
//! ce qui tourne dessus l'est aussi.

use std::collections::HashMap;

use wind_tunnel::geom::Vec2;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::{CellId, Mesh, Side};

const H: f64 = 0.25;

fn tunnel() -> (Mask, Mesh) {
    let mask = Mask::from_file("domains/tunnel.dom").expect("domains/tunnel.dom illisible");
    let mesh = Mesh::from_mask(&mask, H).expect("maillage impossible");
    (mask, mesh)
}

#[test]
fn total_area_matches_the_mask() {
    let (mask, mesh) = tunnel();
    // chaque cellule fluide vaut h², sauf celles dont un coin a été chanfreiné (h²/2 en moins)
    let (tri, _) = mesh.shape_counts();
    let expected = mask.fluid_count() as f64 * H * H - tri as f64 * H * H / 2.0;
    assert!(
        (mesh.total_area() - expected).abs() < 1e-9,
        "aire {} ≠ {expected}",
        mesh.total_area()
    );
}

#[test]
fn the_mesh_is_mixed() {
    let (_, mesh) = tunnel();
    let (tri, quad) = mesh.shape_counts();
    assert!(
        tri > 0,
        "aucun triangle : le chanfreinage ne s'applique pas"
    );
    assert!(quad > tri, "le cas courant doit rester le quadrangle");
    assert_eq!(tri + quad, mesh.n_cells());
}

#[test]
fn each_edge_is_seen_once_or_twice() {
    let (_, mesh) = tunnel();
    let mut count: HashMap<(u32, u32), usize> = HashMap::new();
    for cell in mesh.cells() {
        let vertices = cell.kind.vertices();
        for (k, &a) in vertices.iter().enumerate() {
            let b = vertices[(k + 1) % vertices.len()];
            let key = if a.0 < b.0 { (a.0, b.0) } else { (b.0, a.0) };
            *count.entry(key).or_default() += 1;
        }
    }
    assert!(count.values().all(|&n| n == 1 || n == 2));
    assert_eq!(count.len(), mesh.n_faces());

    let inner = mesh.faces().iter().filter(|f| !f.is_boundary()).count();
    assert_eq!(inner, count.values().filter(|&&n| n == 2).count());
}

#[test]
fn oriented_normals_sum_to_zero() {
    // NB: Lié à Mesh::build_cell_faces qui est en bonus
    // identité géométrique : ∮ n dl = 0 sur tout contour fermé
    let (_, mesh) = tunnel();
    for i in 0..mesh.n_cells() {
        let id = CellId(i as u32);
        let sum = mesh.cell_faces(id).iter().fold(Vec2::ZERO, |acc, &fid| {
            let f = mesh.face(fid);
            let sign = if f.left == id { 1.0 } else { -1.0 };
            acc + f.normal * (f.length * sign)
        });
        assert!(sum.norm() < 1e-12, "cellule {i} : {sum:?}");
    }
}

#[test]
fn connectivity_is_symmetric() {
    // NB: Lié à Mesh::build_cell_faces qui est en bonus
    // si j'ai une face avec un voisin, ce voisin a la même face dans sa propre liste
    let (_, mesh) = tunnel();
    for i in 0..mesh.n_cells() {
        let id = CellId(i as u32);
        for &fid in mesh.cell_faces(id) {
            let face = mesh.face(fid);
            let other = match face.right {
                Side::Inner(c) if face.left == id => c,
                Side::Inner(_) => face.left,
                Side::Boundary(_) => continue,
            };
            assert!(
                mesh.cell_faces(other).contains(&fid),
                "la face {} manque chez la cellule {}",
                fid.index(),
                other.index()
            );
        }
    }
}

#[test]
fn every_boundary_group_is_populated() {
    let (_, mesh) = tunnel();
    let groups = mesh.groups();
    assert_eq!(groups.len(), 4, "entrée, sortie, parois, obstacle");
    for (kind, faces) in groups {
        assert!(!faces.is_empty(), "groupe {kind:?} vide");
    }
    let boundary_faces: usize = groups.values().map(|f| f.len()).sum();
    assert_eq!(
        boundary_faces,
        mesh.faces().iter().filter(|f| f.is_boundary()).count()
    );
}
