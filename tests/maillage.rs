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

fn veine() -> (Mask, Mesh) {
    let mask = Mask::from_file("domains/veine.dom").expect("domains/veine.dom illisible");
    let mesh = Mesh::from_mask(&mask, H).expect("maillage impossible");
    (mask, mesh)
}

#[test]
fn l_aire_totale_correspond_au_masque() {
    let (mask, mesh) = veine();
    // chaque cellule fluide vaut h², sauf celles dont un coin a été chanfreiné (h²/2 en moins)
    let (tri, _) = mesh.shape_counts();
    let attendu = mask.fluid_count() as f64 * H * H - tri as f64 * H * H / 2.0;
    assert!(
        (mesh.total_area() - attendu).abs() < 1e-9,
        "aire {} ≠ {attendu}",
        mesh.total_area()
    );
}

#[test]
fn le_maillage_est_mixte() {
    let (_, mesh) = veine();
    let (tri, quad) = mesh.shape_counts();
    assert!(
        tri > 0,
        "aucun triangle : le chanfreinage ne s'applique pas"
    );
    assert!(quad > tri, "le cas courant doit rester le quadrangle");
    assert_eq!(tri + quad, mesh.n_cells());
}

#[test]
fn chaque_arete_est_vue_une_ou_deux_fois() {
    let (_, mesh) = veine();
    let mut compte: HashMap<(u32, u32), usize> = HashMap::new();
    for cell in mesh.cells() {
        let sommets = cell.kind.vertices();
        for (k, &a) in sommets.iter().enumerate() {
            let b = sommets[(k + 1) % sommets.len()];
            let cle = if a.0 < b.0 { (a.0, b.0) } else { (b.0, a.0) };
            *compte.entry(cle).or_default() += 1;
        }
    }
    assert!(compte.values().all(|&n| n == 1 || n == 2));
    assert_eq!(compte.len(), mesh.n_faces());

    let internes = mesh.faces().iter().filter(|f| !f.is_boundary()).count();
    assert_eq!(internes, compte.values().filter(|&&n| n == 2).count());
}

#[test]
fn la_somme_des_normales_orientees_est_nulle() {
    // identité géométrique : ∮ n dl = 0 sur tout contour fermé
    let (_, mesh) = veine();
    for i in 0..mesh.n_cells() {
        let id = CellId(i as u32);
        let somme = mesh.cell_faces(id).iter().fold(Vec2::ZERO, |acc, &fid| {
            let f = mesh.face(fid);
            let signe = if f.left == id { 1.0 } else { -1.0 };
            acc + f.normal * (f.length * signe)
        });
        assert!(somme.norm() < 1e-12, "cellule {i} : {somme:?}");
    }
}

#[test]
fn la_connectivite_est_symetrique() {
    // si j'ai une face avec un voisin, ce voisin a la même face dans sa propre liste
    let (_, mesh) = veine();
    for i in 0..mesh.n_cells() {
        let id = CellId(i as u32);
        for &fid in mesh.cell_faces(id) {
            let face = mesh.face(fid);
            let autre = match face.right {
                Side::Inner(c) if face.left == id => c,
                Side::Inner(_) => face.left,
                Side::Boundary(_) => continue,
            };
            assert!(
                mesh.cell_faces(autre).contains(&fid),
                "la face {} manque chez la cellule {}",
                fid.index(),
                autre.index()
            );
        }
    }
}

#[test]
fn tous_les_groupes_de_bord_sont_peuples() {
    let (_, mesh) = veine();
    let groupes = mesh.groups();
    assert_eq!(groupes.len(), 4, "entrée, sortie, parois, obstacle");
    for (nature, faces) in groupes {
        assert!(!faces.is_empty(), "groupe {nature:?} vide");
    }
    let bords: usize = groupes.values().map(|f| f.len()).sum();
    assert_eq!(
        bords,
        mesh.faces().iter().filter(|f| f.is_boundary()).count()
    );
}
