#![cfg(feature = "step3")]

//! Le fichier écrit doit être relisible par un outil qui n'est pas le nôtre.
//!
//! On ne peut pas lancer Paraview depuis un test, mais on peut vérifier que la
//! structure du fichier est exactement celle que le format annonce : autant de points
//! que de sommets, autant de types que de cellules, autant de valeurs que de cellules.
//! C'est ce que Paraview vérifiera aussi, en moins bavard.

use std::fs;

use wind_tunnel::field::Field;
#[cfg(feature = "step4")]
use wind_tunnel::geom::Vec2;
use wind_tunnel::io::vtk::write_vtk;
#[cfg(feature = "step4")]
use wind_tunnel::io::vtk::{write_frame, Frame};
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

// La fonction de courant n'a de sens qu'à partir de l'étape 4 : ce test n'a rien à faire
// dans les rouges de l'étape 3, où le stagiaire n'a jamais entendu parler de `ψ`.
#[cfg(feature = "step4")]
#[test]
fn a_frame_carries_the_flow_and_its_date() {
    let mesh = small_mesh();
    let c = Field::from_fn(&mesh, |id| id.index() as f64);
    let speed = Field::filled(mesh.n_cells(), 2.0);
    let velocity = vec![Vec2::new(2.0, 0.0); mesh.n_cells()];
    let psi: Vec<f64> = mesh.vertices().iter().map(|p| p.y).collect();

    let path = std::env::temp_dir().join("wind-tunnel-test-frame.vtk");
    write_frame(
        &path,
        &mesh,
        &Frame {
            cells: &[("c", &c), ("speed", &speed)],
            vectors: &[("u", &velocity)],
            points: &[("psi", &psi)],
            time: 1.25,
            cycle: 7,
        },
    )
    .expect("écriture impossible");
    let text = fs::read_to_string(&path).expect("relecture impossible");

    // Les champs aux cellules d'abord, les sommets ensuite : l'ordre des sections compte,
    // un lecteur legacy rattache chaque tableau à la dernière section ouverte.
    let cell_data = text.find("CELL_DATA").expect("CELL_DATA absent");
    let point_data = text.find("POINT_DATA").expect("POINT_DATA absent");
    assert!(text.find("VECTORS u double").expect("vecteurs absents") > cell_data);
    assert!(point_data > cell_data);
    assert_eq!(header_value(&text, "POINT_DATA"), mesh.n_vertices());

    // autant de valeurs de ψ que de sommets, et autant de vecteurs que de cellules
    let psi_values = text[point_data..]
        .lines()
        .skip(3)
        .take_while(|l| l.parse::<f64>().is_ok())
        .count();
    assert_eq!(psi_values, mesh.n_vertices());

    // la date, sous les trois noms attendus par les lecteurs usuels
    for keyword in ["TIME 1 1 double", "TimeValue 1 1 double", "CYCLE 1 1 int"] {
        assert!(text.contains(keyword), "{keyword} absent");
    }
    let time = text
        .lines()
        .skip_while(|l| !l.starts_with("TIME 1 1 double"))
        .nth(1)
        .expect("valeur de temps absente");
    assert_eq!(time.parse::<f64>().expect("temps illisible"), 1.25);

    fs::remove_file(&path).ok();
}
