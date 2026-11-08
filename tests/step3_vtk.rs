#![cfg(feature = "step3")]

//! Le fichier écrit doit être relisible par un outil qui n'est pas le nôtre.
//!
//! On ne peut pas lancer Paraview depuis un test, mais on peut vérifier que la
//! structure du fichier est exactement celle que le format annonce : autant de points
//! que de sommets, autant de types que de cellules, autant de valeurs que de cellules.
//! C'est ce que Paraview vérifiera aussi, en moins bavard.
//!
//! Les deux derniers tests écrivent une image complète — vitesse aux cellules, `ψ` aux
//! sommets, date — puis la série qui date chaque image. Ils passent par le même trou,
//! `write_dataset` ; ce que sont `u` et `ψ` n'importe pas ici, et viendra à l'étape 4.

mod common;

use std::fs;

use common::{header_value, small_mesh, temp_path};
use wind_tunnel::field::Field;
use wind_tunnel::geom::Vec2;
use wind_tunnel::io::vtk::{Frame, write_frame, write_series, write_vtk};
use wind_tunnel::mesh::CellId;

#[test]
fn the_vtk_file_describes_the_whole_mesh() {
    let mesh = small_mesh();
    let field = Field::from_fn(&mesh, |id| id.index() as f64);

    let path = temp_path("mesh.vtk");
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

    let path = temp_path("denormal.vtk");
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

    let path = temp_path("field.vtk");
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

#[test]
fn a_frame_carries_the_flow_and_its_date() {
    let mesh = small_mesh();
    let c = Field::from_fn(&mesh, |id| id.index() as f64);
    let speed = Field::filled(mesh.n_cells(), 2.0);
    let velocity = vec![Vec2::new(2.0, 0.0); mesh.n_cells()];
    let psi: Vec<f64> = mesh.vertices().iter().map(|p| p.y).collect();

    let path = temp_path("frame.vtk");
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

    // La date, sous les trois noms attendus par les lecteurs usuels — et surtout **avant**
    // les POINTS. Écrite plus bas, elle est rattachée à la dernière section ouverte : le
    // lecteur y voit un tableau d'une seule valeur pour N points, refuse le fichier entier
    // (« Attribute Mismatch ») et ParaView n'affiche plus rien. C'est arrivé.
    for keyword in ["TIME 1 1 double", "TimeValue 1 1 double", "CYCLE 1 1 int"] {
        assert!(text.contains(keyword), "{keyword} absent");
    }
    let field = text.find("FIELD FieldData").expect("FIELD absent");
    let points = text.find("POINTS").expect("POINTS absent");
    assert!(
        field < points,
        "le bloc FIELD doit précéder POINTS, sinon le lecteur le rattache aux points"
    );
    let time = text
        .lines()
        .skip_while(|l| !l.starts_with("TIME 1 1 double"))
        .nth(1)
        .expect("valeur de temps absente");
    assert_eq!(time.parse::<f64>().expect("temps illisible"), 1.25);

    fs::remove_file(&path).ok();
}

#[test]
fn the_series_file_dates_every_frame() {
    let path = temp_path("frames.vtk.series");
    let frames = vec![
        ("frame_0000.vtk".to_string(), 0.0),
        ("frame_0001.vtk".to_string(), 0.5),
        ("frame_0002.vtk".to_string(), 1.25),
    ];
    write_series(&path, &frames).expect("écriture impossible");
    let text = fs::read_to_string(&path).expect("relecture impossible");

    assert!(text.contains("\"file-series-version\": \"1.0\""));
    for (name, time) in &frames {
        assert!(
            text.contains(&format!("\"name\": \"{name}\"")),
            "{name} absent de la série"
        );
        assert!(
            text.contains(&format!("\"time\": {time}")),
            "date de {name}"
        );
    }
    // Une virgule de trop après le dernier élément, et le JSON n'est plus lisible.
    assert!(!text.contains("}},\n  ]"), "virgule finale en trop");

    fs::remove_file(&path).ok();
}
