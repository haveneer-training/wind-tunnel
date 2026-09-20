//! Les tests de l'exercice de conception. Ils sont **donnés** : ils fixent les noms
//! publics et le contrat, et ne compilent pas tant que `src/lib.rs` est vide.
//!
//! Lisez les erreurs du compilateur dans l'ordre : chacune nomme une définition qui
//! manque encore.

use wind_tunnel_design::{neighbours, BoundaryKind, CellId, Face, Side, VertexId};

/// Une face interne, entre les cellules 7 et 9.
fn inner_face() -> Face {
    Face {
        a: VertexId(0),
        b: VertexId(1),
        left: CellId(7),
        right: Side::Inner(CellId(9)),
    }
}

/// Une face de paroi, du côté de la cellule 7.
fn wall_face() -> Face {
    Face {
        a: VertexId(1),
        b: VertexId(2),
        left: CellId(7),
        right: Side::Boundary(BoundaryKind::Wall),
    }
}

#[test]
fn an_identifier_is_an_index() {
    assert_eq!(CellId(7).index(), 7usize);
    assert_eq!(VertexId(0).index(), 0usize);
}

#[test]
fn an_inner_face_links_its_two_cells() {
    let face = inner_face();
    assert!(!face.is_boundary());
    assert_eq!(face.neighbour(CellId(7)), Some(CellId(9)));
    assert_eq!(face.neighbour(CellId(9)), Some(CellId(7)));
}

#[test]
fn a_boundary_face_has_no_neighbour() {
    let face = wall_face();
    assert!(face.is_boundary());
    assert_eq!(face.neighbour(CellId(7)), None);
}

#[test]
fn a_face_ignores_a_cell_it_does_not_touch() {
    assert_eq!(inner_face().neighbour(CellId(42)), None);
}

#[test]
fn every_side_is_either_inner_or_boundary() {
    // Ce `match` n'a **pas** de bras `_`. Il ne compile que si `Side` a exactement deux
    // possibilités, et il cesserait de compiler si une troisième apparaissait — c'est
    // précisément ce qu'on attend d'une énumération : que le compilateur rappelle tous
    // les cas à traiter, plutôt qu'un drapeau qu'on oublie de tester.
    for side in [Side::Inner(CellId(1)), Side::Boundary(BoundaryKind::Outlet)] {
        let described = match side {
            Side::Inner(id) => format!("cellule {}", id.index()),
            Side::Boundary(kind) => format!("bord {kind:?}"),
        };
        assert!(!described.is_empty());
    }
}

#[test]
fn boundary_kinds_are_distinct() {
    assert_ne!(BoundaryKind::Inlet, BoundaryKind::Outlet);
    assert_eq!(BoundaryKind::Wall, BoundaryKind::Wall);
}

#[test]
fn the_enum_costs_no_more_than_a_flag() {
    // Un `u32` de charge utile et de quoi dire laquelle des deux possibilités :
    // 8 octets. La version « à drapeau » — `Option<CellId>` plus un `bool` plus le type
    // de frontière — en demanderait davantage, pour une garantie moindre. La sûreté ne
    // se paie pas ici.
    assert!(
        std::mem::size_of::<Side>() <= 8,
        "Side occupe {} octets",
        std::mem::size_of::<Side>()
    );
}

#[test]
fn the_neighbours_of_a_cell() {
    let faces = [inner_face(), wall_face()];
    assert_eq!(neighbours(&faces, CellId(7)), vec![CellId(9)]);
    assert_eq!(neighbours(&faces, CellId(9)), vec![CellId(7)]);
    assert!(neighbours(&faces, CellId(42)).is_empty());

    // `faces` est toujours utilisable : `neighbours` ne l'a qu'emprunté.
    assert_eq!(faces.len(), 2);
}
