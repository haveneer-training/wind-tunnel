#![cfg(feature = "step4")]

//! Une image complète du calcul : le traceur, mais aussi l'écoulement de l'étape 4
//! (vitesse aux cellules, `ψ` aux sommets) et la date, puis la série qui date chaque
//! image pour ParaView.
//!
//! La fonction de courant n'a de sens qu'à partir de l'étape 4 : ces tests n'ont rien à
//! faire dans les rouges de l'étape 3, où le stagiaire n'a jamais entendu parler de `ψ`.

mod common;

use std::fs;

use common::{header_value, small_mesh, temp_path};
use wind_tunnel::field::Field;
use wind_tunnel::geom::Vec2;
use wind_tunnel::io::vtk::{Frame, write_frame, write_series};

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
