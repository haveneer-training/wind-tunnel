#![cfg(feature = "step3")]

//! Le fichier écrit doit être relisible par un outil qui n'est pas le nôtre.
//!
//! On ne peut pas lancer Paraview depuis un test, mais on peut vérifier que la
//! structure du fichier est exactement celle que le format annonce : autant de points
//! que de sommets, autant de types que de cellules, autant de valeurs que de cellules.
//! C'est ce que Paraview vérifiera aussi, en moins bavard.

use std::fs;

use wind_tunnel::field::Field;
use wind_tunnel::io::vtk::write_vtk;
use wind_tunnel::mask::Mask;
use wind_tunnel::mesh::{CellId, Mesh};

fn small_mesh() -> Mesh {
    let mask = Mask::parse("....\n.##.\n....\n").expect("masque invalide");
    Mesh::from_mask(&mask, 0.5).expect("maillage impossible")
}

/// Valeur qui suit un mot-clé dans l'en-tête VTK, par exemple `POINTS 42 double`.
fn header_value(text: &str, keyword: &str) -> usize {
    text.lines()
        .find(|l| l.starts_with(keyword))
        .unwrap_or_else(|| panic!("mot-clé {keyword} absent du fichier"))
        .split_whitespace()
        .nth(1)
        .expect("valeur absente")
        .parse()
        .expect("valeur non entière")
}

#[test]
fn the_vtk_file_describes_the_whole_mesh() {
    let mesh = small_mesh();
    let field = Field::from_fn(&mesh, |id| id.index() as f64);

    let path = std::env::temp_dir().join("wind-tunnel-test-mesh.vtk");
    write_vtk(&path, &mesh, &[("c", &field)]).expect("écriture impossible");
    let text = fs::read_to_string(&path).expect("relecture impossible");

    assert!(text.starts_with("# vtk DataFile Version 3.0"));
    assert!(text.contains("DATASET UNSTRUCTURED_GRID"));
    assert_eq!(header_value(&text, "POINTS"), mesh.n_vertices());
    assert_eq!(header_value(&text, "CELLS"), mesh.n_cells());
    assert_eq!(header_value(&text, "CELL_TYPES"), mesh.n_cells());
    assert_eq!(header_value(&text, "CELL_DATA"), mesh.n_cells());

    // le second nombre de la ligne CELLS annonce le total d'entiers qui suivent
    let announced: usize = text
        .lines()
        .find(|l| l.starts_with("CELLS"))
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    let written: usize = mesh
        .cells()
        .iter()
        .map(|c| c.kind.vertices().len() + 1)
        .sum();
    assert_eq!(announced, written);

    fs::remove_file(&path).ok();
}

/// Le traceur décroît exponentiellement loin de sa source : en quelques dizaines de pas,
/// les cellules du fond portent des valeurs *dénormales* (sous `f64::MIN_POSITIVE`). Le
/// lecteur VTK legacy les lit par `istream >> double`, qui échoue au sous-débordement,
/// puis abandonne le reste du fichier — Paraview n'affiche plus rien. Elles doivent donc
/// sortir à zéro, ce qu'elles sont à toutes fins utiles.
#[test]
fn denormal_values_are_written_as_zero() {
    let mesh = small_mesh();
    // dénormal, dénormal négatif, normal minuscule, ordinaire
    let probes = [3.5e-323, -7e-323, 2.196_746_312_594_583e-303, 0.25];
    let field = Field::from_fn(&mesh, |id| probes[id.index() % probes.len()]);

    let path = std::env::temp_dir().join("wind-tunnel-test-denormal.vtk");
    write_vtk(&path, &mesh, &[("c", &field)]).expect("écriture impossible");
    let text = fs::read_to_string(&path).expect("relecture impossible");

    let values: Vec<f64> = text
        .lines()
        .skip_while(|l| !l.starts_with("LOOKUP_TABLE"))
        .skip(1)
        .filter_map(|l| l.trim().parse().ok())
        .collect();
    assert_eq!(values.len(), mesh.n_cells());

    for (i, value) in values.iter().enumerate() {
        let expected = field[CellId(i as u32)];
        if expected.abs() < f64::MIN_POSITIVE {
            assert_eq!(
                *value, 0.0,
                "dénormal {expected:e} écrit tel quel (cellule {i})"
            );
        } else {
            // Écourter les autres ne doit rien perdre : elles se relisent à l'identique.
            assert_eq!(*value, expected, "valeur {i} altérée");
        }
    }

    fs::remove_file(&path).ok();
}

#[test]
fn the_field_values_are_written_in_cell_order() {
    let mesh = small_mesh();
    let field = Field::from_fn(&mesh, |id| 10.0 * id.index() as f64);

    let path = std::env::temp_dir().join("wind-tunnel-test-field.vtk");
    write_vtk(&path, &mesh, &[("c", &field)]).expect("écriture impossible");
    let text = fs::read_to_string(&path).expect("relecture impossible");

    let values: Vec<f64> = text
        .lines()
        .skip_while(|l| !l.starts_with("LOOKUP_TABLE"))
        .skip(1)
        .filter_map(|l| l.trim().parse().ok())
        .collect();

    assert_eq!(values.len(), mesh.n_cells());
    for (i, value) in values.iter().enumerate() {
        assert_eq!(*value, field[CellId(i as u32)], "valeur {i} désordonnée");
    }

    fs::remove_file(&path).ok();
}
